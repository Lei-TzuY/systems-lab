//go:build linux

package container

import (
	"errors"
	"fmt"

	"minicontainer/internal/dns"
	"minicontainer/internal/network"
	"minicontainer/internal/state"
)

type ownedPortCleanupFunc func(owner string, hostPort, containerPort int, containerIP, protocol string, debug bool) error
type ownedVethCleanupFunc func(name, owner string, debug bool) error

func networkOwnershipForGeneration(owner string, pid int, pidStartTime uint64, containerIP string, mappings []PortMapping) state.NetworkOwnership {
	owned := state.NetworkOwnership{
		Owner:        owner,
		PID:          pid,
		PIDStartTime: pidStartTime,
		VethHost:     network.VethHostIfaceOwned(owner),
		Mappings:     make([]state.PortForwardingOwnership, 0, len(mappings)),
	}
	for _, mapping := range mappings {
		protocol := normalizedProtocol(mapping.Protocol)
		owned.Mappings = append(owned.Mappings, state.PortForwardingOwnership{
			HostPort:      mapping.HostPort,
			ContainerPort: mapping.ContainerPort,
			ContainerIP:   containerIP,
			Protocol:      protocol,
		})
	}
	return owned
}

func cleanupNetworkOwnershipWith(
	st *state.Store,
	containerID string,
	ownership state.NetworkOwnership,
	debug bool,
	removePort ownedPortCleanupFunc,
	removeVeth ownedVethCleanupFunc,
) error {
	if st == nil {
		return fmt.Errorf("state store is nil")
	}
	if removePort == nil || removeVeth == nil {
		return fmt.Errorf("owned network cleanup function is nil")
	}

	var cleanupErrs []error
	for i := len(ownership.Mappings) - 1; i >= 0; i-- {
		mapping := ownership.Mappings[i]
		if err := removePort(
			ownership.Owner,
			mapping.HostPort,
			mapping.ContainerPort,
			mapping.ContainerIP,
			mapping.Protocol,
			debug,
		); err != nil {
			cleanupErrs = append(cleanupErrs, fmt.Errorf(
				"remove persisted port mapping %d:%d/%s: %w",
				mapping.HostPort,
				mapping.ContainerPort,
				mapping.Protocol,
				err,
			))
		}
	}
	if ownership.VethHost != "" {
		if err := removeVeth(ownership.VethHost, ownership.Owner, debug); err != nil {
			cleanupErrs = append(cleanupErrs, fmt.Errorf("remove persisted host veth %s: %w", ownership.VethHost, err))
		}
	}
	if err := errors.Join(cleanupErrs...); err != nil {
		return err
	}

	cleared, err := st.ClearNetworkOwnershipIfMatch(containerID, ownership)
	if err != nil {
		return fmt.Errorf("clear network ownership after cleanup: %w", err)
	}
	if cleared {
		return nil
	}
	if _, ok, err := st.GetNetworkOwnership(containerID); err != nil {
		return fmt.Errorf("re-read network ownership after cleanup: %w", err)
	} else if !ok {
		// Another lifecycle actor completed the same idempotent cleanup first.
		return nil
	}
	return fmt.Errorf("network ownership changed or remained after successful cleanup")
}

// cleanupNetworkOwnershipAfterDurableStopWith is the destructive-cleanup gate
// for generic callers that may run before authoritative lifecycle finalization.
// A running/created record is a durable claim that host networking may still be
// in use, so cleanup must be a no-op until stopped has committed. Once stopped,
// the sidecar process identity must match the exact durable exited generation;
// stale or cross-record ownership must never gain destructive cleanup authority.
// State read failures fail closed and preserve both host resources and the
// ownership token.
func cleanupNetworkOwnershipAfterDurableStopWith(
	st *state.Store,
	containerID string,
	ownership state.NetworkOwnership,
	debug bool,
	removePort ownedPortCleanupFunc,
	removeVeth ownedVethCleanupFunc,
) error {
	if st == nil {
		return fmt.Errorf("state store is nil")
	}
	current, err := st.Get(containerID)
	if err != nil {
		return fmt.Errorf("read lifecycle state before network cleanup for container %s: %w", containerID, err)
	}
	if current.Status != state.StatusStopped {
		return nil
	}

	exitedPID, exitedStartTime, revisionCurrent, identityOK, identityRequired, err := st.GetStoppedExitIdentityPolicy(containerID, current.Revision)
	if err != nil {
		return fmt.Errorf("read stopped generation identity before network cleanup for container %s: %w", containerID, err)
	}
	if !revisionCurrent {
		// A concurrent lifecycle transition invalidated the snapshot used to
		// authorize cleanup. The new generation owns the decision.
		return nil
	}
	if !identityOK {
		if identityRequired {
			return fmt.Errorf("stopped container %s revision %d is missing required exited process identity while network ownership remains", containerID, current.Revision)
		}
		return fmt.Errorf("refusing network cleanup for stopped container %s revision %d without exact exited process identity", containerID, current.Revision)
	}
	if ownership.PID != exitedPID || ownership.PIDStartTime != exitedStartTime {
		return fmt.Errorf(
			"refusing network cleanup for container %s: ownership belongs to process %d/%d, stopped generation is %d/%d",
			containerID,
			ownership.PID,
			ownership.PIDStartTime,
			exitedPID,
			exitedStartTime,
		)
	}
	return cleanupNetworkOwnershipWith(st, containerID, ownership, debug, removePort, removeVeth)
}

func cleanupNetworkOwnership(st *state.Store, containerID string, ownership state.NetworkOwnership, debug bool) error {
	return cleanupNetworkOwnershipAfterDurableStopWith(
		st,
		containerID,
		ownership,
		debug,
		network.RemovePortForwardingOwned,
		network.RemoveVethHostOwned,
	)
}

// CleanupStoppedNetwork retries generation-owned host-network cleanup after a
// parent crash or an earlier teardown failure. Legacy containers without a
// network sidecar are no-ops; rules-only sidecars from older runtimes remain
// supported.
func CleanupStoppedNetwork(st *state.Store, c *state.Container) error {
	if st == nil {
		return fmt.Errorf("state store is nil")
	}
	if c == nil {
		return fmt.Errorf("container snapshot is nil")
	}
	if c.ID == "" {
		return fmt.Errorf("container ID is empty")
	}
	if c.Status != state.StatusStopped {
		return fmt.Errorf("container %s is %s; network cleanup retry requires stopped state", c.ID, c.Status)
	}
	current, err := stoppedSnapshotStillCurrent(st, c)
	if err != nil {
		return fmt.Errorf("validate stopped cleanup snapshot for container %s: %w", c.ID, err)
	}
	if !current {
		return nil
	}

	ownership, ok, err := st.GetNetworkOwnership(c.ID)
	if err != nil {
		return fmt.Errorf("read network ownership for stopped container %s: %w", c.ID, err)
	}
	if !ok {
		return nil
	}
	if err := cleanupNetworkOwnership(st, c.ID, ownership, false); err != nil {
		return fmt.Errorf("cleanup persisted network resources for stopped container %s: %w", c.ID, err)
	}
	return nil
}

func cleanupStoppedDNSRegistration(st *state.Store, c *state.Container) error {
	pid, pidStartTime, current, ok, required, err := st.GetStoppedExitIdentityPolicy(c.ID, c.Revision)
	if err != nil {
		return fmt.Errorf("read stopped generation identity policy for DNS cleanup: %w", err)
	}
	if !current {
		return nil
	}

	if !ok {
		if required {
			return fmt.Errorf("stopped container %s revision %d is missing required exited process identity", c.ID, c.Revision)
		}

		// Historical stopped records may predate durable exited identity. Only
		// records without the durable capability marker can acquire this migration
		// authority. Identity absence and compatibility policy were read under one
		// state lock, so cleanup cannot classify a different stopped generation.
		// Reconcile both legacy DNS ownership classes in one registry transaction:
		// ownerless debris is retired immediately, registrar-owned records only
		// when their owner is inactive, and modern registrations remain outside
		// this migration-only authority.
		return dns.CleanupStoppedLegacyHostRegistrations(defaultBridgeDNSNetwork, c.ID)
	}

	// The stopped revision and durable exited identity authorize one DNS registry
	// transaction. It retires the exact modern generation plus any historical
	// generation-unaware debris while preserving unbound or newer modern attempts.
	return dns.CleanupStoppedHostRegistrationGeneration(defaultBridgeDNSNetwork, c.ID, pid, pidStartTime)
}

// CleanupStoppedRuntimeResources retries every durable host-side cleanup token
// currently known for a stopped generation. Independent failures are joined so
// one resource class cannot prevent another from making progress. Modern DNS
// teardown reads the stopped revision, durable PID/start-time identity, and
// legacy-compatibility policy atomically. A modern stopped record missing that
// required sidecar fails closed; only records that predate the durable capability
// marker may use legacy DNS migration recovery.
func CleanupStoppedRuntimeResources(st *state.Store, c *state.Container) error {
	if c == nil {
		return errors.Join(CleanupStoppedCgroup(st, c), CleanupStoppedNetwork(st, c))
	}
	current, err := stoppedSnapshotStillCurrent(st, c)
	if err != nil {
		return fmt.Errorf("validate stopped runtime cleanup snapshot for container %s: %w", c.ID, err)
	}
	if !current {
		return nil
	}
	return errors.Join(
		CleanupStoppedCgroup(st, c),
		CleanupStoppedNetwork(st, c),
		cleanupStoppedDNSRegistration(st, c),
	)
}
