//go:build linux

package container

import (
	"fmt"
	"os"

	"minicontainer/internal/dns"
	"minicontainer/internal/events"
	"minicontainer/internal/state"
)

const defaultBridgeDNSNetwork = "default"
const defaultBridgeContainerIP = "172.20.0.2"

type networkAdmissionDeps struct {
	validateDNSRootFS func(rootfsPath, networkName string) error
	beginDNSAttempt   func(networkName, containerID, hostname, ipAddr string) (func() error, error)
	registerDNSHost   func(networkName, containerID, hostname, ipAddr string) error
	unregisterDNSHost func(networkName, containerID string) error
}

func defaultNetworkAdmissionDeps() networkAdmissionDeps {
	return networkAdmissionDeps{
		validateDNSRootFS: dns.InjectHostsIntoRootFS,
		beginDNSAttempt:   dns.BeginHostRegistrationAttempt,
		registerDNSHost:   dns.RegisterHost,
		unregisterDNSHost: dns.UnregisterHostOwned,
	}
}

func validateAdmittedRootFSIdentity(cfg Config) error {
	if cfg.RootFSIdentity == nil {
		return nil
	}
	current, err := os.Stat(cfg.RootFS)
	if err != nil {
		return &runtimeSetupError{err: fmt.Errorf("revalidate admitted rootfs %q: %w", cfg.RootFS, err)}
	}
	if !current.IsDir() {
		return &runtimeSetupError{err: fmt.Errorf("revalidate admitted rootfs %q: no longer a directory", cfg.RootFS)}
	}
	if !os.SameFile(cfg.RootFSIdentity, current) {
		return &runtimeSetupError{err: fmt.Errorf("revalidate admitted rootfs %q: filesystem identity changed before runtime attempt", cfg.RootFS)}
	}
	return nil
}

// requireDurableNetworkOwnership preserves the narrow validation API used by
// focused tests. Production Run uses beginNetworkAttemptAdmission so each
// restart attempt receives its own DNS registration and owned rollback token.
func requireDurableNetworkOwnership(cfg Config, lifecycleStore *state.Store) error {
	return requireDurableNetworkOwnershipWith(cfg, lifecycleStore, defaultNetworkAdmissionDeps())
}

func requireDurableNetworkOwnershipWith(cfg Config, lifecycleStore *state.Store, deps networkAdmissionDeps) error {
	rollback, err := beginNetworkAttemptAdmissionWith(cfg, lifecycleStore, deps)
	if err != nil {
		return err
	}
	if rollback == nil {
		return nil
	}
	if err := rollback(); err != nil {
		return &runtimeSetupError{err: fmt.Errorf("rollback network validation admission: %w", err)}
	}
	return nil
}

// rollbackNetworkAdmissionAfterRun consumes an attempt-scoped DNS admission
// only while durable lifecycle state still proves that no process generation
// was admitted. Once a generation reached stopped, authoritative generation
// finalization owns DNS teardown; replaying this attempt-scoped rollback is
// unnecessary. A running, stopped, or unreadable record therefore preserves the
// entry here. State read failures fail closed so a later lifecycle actor can
// reconcile it safely.
func rollbackNetworkAdmissionAfterRun(lifecycleStore *state.Store, containerID string, rollback func() error) error {
	if rollback == nil {
		return nil
	}
	if lifecycleStore == nil {
		return fmt.Errorf("lifecycle store is nil while rolling back bridge DNS admission")
	}
	current, err := lifecycleStore.Get(containerID)
	if err != nil {
		return fmt.Errorf("read lifecycle state before bridge DNS rollback: %w", err)
	}
	switch current.Status {
	case state.StatusCreated:
		return rollback()
	case state.StatusRunning, state.StatusStopped:
		return nil
	default:
		return fmt.Errorf("refuse bridge DNS rollback for container %s with unknown lifecycle state %q", containerID, current.Status)
	}
}

// beginNetworkAttemptAdmission establishes the process-local Start proof for
// every managed runtime attempt, then performs durable network admission. The
// returned rollback always clears an uncommitted Start proof, including paths
// that fail before a child generation exists. Network admission itself is only
// consumed once durable state proves the attempt is not running. A matching CLI
// pre-stage is an idempotent handoff rather than a second lifecycle authority.
func beginNetworkAttemptAdmission(cfg Config, lifecycleStore *state.Store) (func() error, error) {
	if err := validateAdmittedRootFSIdentity(cfg); err != nil {
		return nil, err
	}
	if cfg.ContainerID == "" {
		return beginNetworkAttemptAdmissionWith(cfg, lifecycleStore, defaultNetworkAdmissionDeps())
	}
	if err := events.StageRuntimeStart(cfg.ContainerID, cfg.RootFS, "started container"); err != nil {
		return nil, &runtimeSetupError{err: fmt.Errorf("stage runtime start event: %w", err)}
	}

	networkRollback, err := beginNetworkAttemptAdmissionWith(cfg, lifecycleStore, defaultNetworkAdmissionDeps())
	if err != nil {
		events.CancelPendingStart(cfg.ContainerID)
		return nil, err
	}
	rollback := func() error {
		events.CancelPendingStart(cfg.ContainerID)
		if networkRollback == nil {
			return nil
		}
		return rollbackNetworkAdmissionAfterRun(lifecycleStore, cfg.ContainerID, networkRollback)
	}
	return rollback, nil
}

// beginNetworkAttemptAdmissionWith fails closed before process creation whenever
// host networking can create resources that outlive the runtime parent. Bridge
// veth ownership and published-port DNAT ownership both require a managed state
// store so a later lifecycle actor can safely recover after a parent crash.
//
// Bridge service discovery is attempt-scoped: every restart attempt validates
// and registers independently, and receives an exact-attempt rollback. The
// production dependency uses an opaque token so stale rollback from one attempt
// cannot consume a newer identical registration from the same registrar. Legacy
// injected register/unregister callbacks remain supported for focused tests.
func beginNetworkAttemptAdmissionWith(cfg Config, lifecycleStore *state.Store, deps networkAdmissionDeps) (func() error, error) {
	if len(cfg.PortMappings) > 0 && !cfg.BridgeNetwork {
		return nil, &runtimeSetupError{err: fmt.Errorf("published ports require bridge networking")}
	}
	if !cfg.BridgeNetwork && len(cfg.PortMappings) == 0 {
		return nil, nil
	}
	if lifecycleStore == nil {
		if cfg.BridgeNetwork {
			return nil, &runtimeStateError{err: fmt.Errorf("bridge networking requires managed lifecycle state for durable network cleanup")}
		}
		return nil, &runtimeStateError{err: fmt.Errorf("published ports require managed lifecycle state for durable network cleanup")}
	}
	if !cfg.BridgeNetwork {
		return nil, nil
	}
	if cfg.ContainerID == "" {
		return nil, &runtimeStateError{err: fmt.Errorf("bridge networking requires a managed container ID")}
	}
	if deps.validateDNSRootFS == nil || (deps.beginDNSAttempt == nil && (deps.registerDNSHost == nil || deps.unregisterDNSHost == nil)) {
		return nil, &runtimeSetupError{err: fmt.Errorf("bridge DNS admission dependencies are incomplete")}
	}
	if err := deps.validateDNSRootFS(cfg.RootFS, defaultBridgeDNSNetwork); err != nil {
		return nil, &runtimeSetupError{err: fmt.Errorf("validate bridge DNS rootfs: %w", err)}
	}

	if deps.beginDNSAttempt != nil {
		dnsRollback, err := deps.beginDNSAttempt(defaultBridgeDNSNetwork, cfg.ContainerID, cfg.Hostname, defaultBridgeContainerIP)
		if err != nil {
			return nil, &runtimeSetupError{err: fmt.Errorf("register bridge DNS host: %w", err)}
		}
		if dnsRollback == nil {
			return nil, &runtimeSetupError{err: fmt.Errorf("register bridge DNS host returned nil attempt rollback")}
		}
		return func() error {
			if err := dnsRollback(); err != nil {
				return fmt.Errorf("unregister bridge DNS host: %w", err)
			}
			return nil
		}, nil
	}

	if err := deps.registerDNSHost(defaultBridgeDNSNetwork, cfg.ContainerID, cfg.Hostname, defaultBridgeContainerIP); err != nil {
		return nil, &runtimeSetupError{err: fmt.Errorf("register bridge DNS host: %w", err)}
	}
	rollback := func() error {
		if err := deps.unregisterDNSHost(defaultBridgeDNSNetwork, cfg.ContainerID); err != nil {
			return fmt.Errorf("unregister bridge DNS host: %w", err)
		}
		return nil
	}
	return rollback, nil
}
