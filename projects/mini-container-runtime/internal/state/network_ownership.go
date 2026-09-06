package state

import (
	"encoding/json"
	"fmt"
	"net"
	"os"
	"path/filepath"
	"strings"
)

const (
	networkOwnershipSuffix        = ".network"
	networkOwnershipSchemaVersion = 1
)

// PortForwardingOwnership describes one exact generation-tagged DNAT mapping.
// It contains enough information to reconstruct the owned iptables rule specs
// after the runtime process that created them has crashed.
type PortForwardingOwnership struct {
	HostPort      int    `json:"host_port"`
	ContainerPort int    `json:"container_port"`
	ContainerIP   string `json:"container_ip"`
	Protocol      string `json:"protocol"`
}

// NetworkOwnership is durable cleanup proof for host networking resources owned
// by one exact process generation. VethHost is a generation-scoped interface
// name whose kernel ifalias carries Owner; Mappings are generation-tagged DNAT
// rules. The sidecar is persisted before either resource class is mutated and
// survives until every described resource has been confirmed absent.
type NetworkOwnership struct {
	Owner        string                    `json:"owner"`
	PID          int                       `json:"pid"`
	PIDStartTime uint64                    `json:"pid_start_time"`
	VethHost     string                    `json:"veth_host,omitempty"`
	Mappings     []PortForwardingOwnership `json:"mappings"`
}

type persistedNetworkOwnership struct {
	SchemaVersion int                       `json:"schema_version"`
	ContainerID   string                    `json:"container_id"`
	Owner         string                    `json:"owner"`
	PID           int                       `json:"pid"`
	PIDStartTime  uint64                    `json:"pid_start_time"`
	VethHost      string                    `json:"veth_host,omitempty"`
	Mappings      []PortForwardingOwnership `json:"mappings"`
}

type networkOwnershipEnvelope struct {
	SchemaVersion json.RawMessage           `json:"schema_version"`
	ContainerID   json.RawMessage           `json:"container_id"`
	Owner         string                    `json:"owner"`
	PID           int                       `json:"pid"`
	PIDStartTime  uint64                    `json:"pid_start_time"`
	VethHost      string                    `json:"veth_host,omitempty"`
	Mappings      []PortForwardingOwnership `json:"mappings"`
}

func networkOwnershipPath(containerDir, id string) string {
	return filepath.Join(containerDir, id+networkOwnershipSuffix)
}

func validateNetworkOwner(owner string) error {
	if !strings.HasPrefix(owner, "minicontainer:") {
		return fmt.Errorf("invalid network owner %q", owner)
	}
	if len(owner) <= len("minicontainer:") || len(owner) > 128 {
		return fmt.Errorf("invalid network owner length %d", len(owner))
	}
	for _, r := range owner[len("minicontainer:"):] {
		if !((r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') || (r >= '0' && r <= '9') || r == '-' || r == '_' || r == '.') {
			return fmt.Errorf("invalid network owner character %q", r)
		}
	}
	return nil
}

func validateNetworkVethHost(name string) error {
	const (
		linuxInterfaceNameMax = 15
		ownedVethPrefix       = "vh"
	)
	if len(name) != linuxInterfaceNameMax || !strings.HasPrefix(name, ownedVethPrefix) {
		return fmt.Errorf("invalid network veth host %q", name)
	}
	for _, r := range name[len(ownedVethPrefix):] {
		if !((r >= 'a' && r <= 'z') || (r >= '2' && r <= '7')) {
			return fmt.Errorf("invalid network veth host %q", name)
		}
	}
	return nil
}

func validatePortForwardingOwnership(m PortForwardingOwnership) error {
	if m.HostPort < 1 || m.HostPort > 65535 || m.ContainerPort < 1 || m.ContainerPort > 65535 {
		return fmt.Errorf("invalid port mapping %d:%d", m.HostPort, m.ContainerPort)
	}
	if net.ParseIP(m.ContainerIP) == nil {
		return fmt.Errorf("invalid container IP %q", m.ContainerIP)
	}
	if m.Protocol != "tcp" && m.Protocol != "udp" {
		return fmt.Errorf("invalid port protocol %q", m.Protocol)
	}
	return nil
}

func validateNetworkOwnership(o NetworkOwnership) error {
	if err := validateNetworkOwner(o.Owner); err != nil {
		return err
	}
	if o.PID <= 0 || o.PIDStartTime == 0 {
		return fmt.Errorf("invalid network process identity %d/%d", o.PID, o.PIDStartTime)
	}
	if o.VethHost != "" {
		if err := validateNetworkVethHost(o.VethHost); err != nil {
			return err
		}
	}
	if o.VethHost == "" && len(o.Mappings) == 0 {
		return fmt.Errorf("network ownership has no resources")
	}
	if len(o.Mappings) > 256 {
		return fmt.Errorf("network ownership has too many port mappings: %d", len(o.Mappings))
	}
	seen := make(map[string]struct{}, len(o.Mappings))
	for _, mapping := range o.Mappings {
		if err := validatePortForwardingOwnership(mapping); err != nil {
			return err
		}
		// Host ingress ownership is exclusive by host port + protocol. Including
		// the destination in this key would allow one sidecar to claim multiple
		// competing DNAT targets for the same socket, making recovery ambiguous.
		key := fmt.Sprintf("%d/%s", mapping.HostPort, mapping.Protocol)
		if _, ok := seen[key]; ok {
			return fmt.Errorf("duplicate network host ingress %s", key)
		}
		seen[key] = struct{}{}
	}
	return nil
}

func (s *Store) writeNetworkOwnershipUnlocked(id string, ownership NetworkOwnership) error {
	if err := validateID(id); err != nil {
		return err
	}
	if err := validateNetworkOwnership(ownership); err != nil {
		return err
	}
	persisted := persistedNetworkOwnership{
		SchemaVersion: networkOwnershipSchemaVersion,
		ContainerID:   id,
		Owner:         ownership.Owner,
		PID:           ownership.PID,
		PIDStartTime:  ownership.PIDStartTime,
		VethHost:      ownership.VethHost,
		Mappings:      ownership.Mappings,
	}
	data, err := json.Marshal(persisted)
	if err != nil {
		return fmt.Errorf("marshal network ownership: %w", err)
	}
	return atomicWriteFile(s.ctrDir, networkOwnershipPath(s.ctrDir, id), data)
}

func decodeNetworkOwnership(id string, data []byte) (NetworkOwnership, error) {
	var envelope networkOwnershipEnvelope
	if err := json.Unmarshal(data, &envelope); err != nil {
		return NetworkOwnership{}, fmt.Errorf("unmarshal network ownership: %w", err)
	}

	// Pre-schema sidecars remain readable so upgrades can finish pending cleanup.
	// Once either provenance field is present, require the complete current
	// envelope so partial downgrade/corruption cannot silently regain legacy
	// cleanup authority.
	legacy := envelope.SchemaVersion == nil && envelope.ContainerID == nil
	if !legacy {
		if envelope.SchemaVersion == nil || envelope.ContainerID == nil {
			return NetworkOwnership{}, fmt.Errorf("invalid persisted network ownership provenance: schema_version and container_id must both be present")
		}
		var schemaVersion int
		if err := json.Unmarshal(envelope.SchemaVersion, &schemaVersion); err != nil {
			return NetworkOwnership{}, fmt.Errorf("invalid persisted network ownership schema_version: %w", err)
		}
		if schemaVersion != networkOwnershipSchemaVersion {
			return NetworkOwnership{}, fmt.Errorf("unsupported persisted network ownership schema_version %d", schemaVersion)
		}
		var containerID string
		if err := json.Unmarshal(envelope.ContainerID, &containerID); err != nil {
			return NetworkOwnership{}, fmt.Errorf("invalid persisted network ownership container_id: %w", err)
		}
		if err := validateID(containerID); err != nil {
			return NetworkOwnership{}, fmt.Errorf("invalid persisted network ownership container_id: %w", err)
		}
		if containerID != id {
			return NetworkOwnership{}, fmt.Errorf("network ownership storage key %q does not match persisted container_id %q", id, containerID)
		}
	}

	ownership := NetworkOwnership{
		Owner:         envelope.Owner,
		PID:           envelope.PID,
		PIDStartTime:  envelope.PIDStartTime,
		VethHost:      envelope.VethHost,
		Mappings:      envelope.Mappings,
	}
	if err := validateNetworkOwnership(ownership); err != nil {
		return NetworkOwnership{}, fmt.Errorf("invalid persisted network ownership: %w", err)
	}
	return ownership, nil
}

func (s *Store) readNetworkOwnershipUnlocked(id string) (NetworkOwnership, bool, error) {
	if err := validateID(id); err != nil {
		return NetworkOwnership{}, false, err
	}
	data, err := readRegularStateFile(networkOwnershipPath(s.ctrDir, id), "network ownership")
	if err != nil {
		if os.IsNotExist(err) {
			return NetworkOwnership{}, false, nil
		}
		return NetworkOwnership{}, false, fmt.Errorf("read network ownership: %w", err)
	}
	ownership, err := decodeNetworkOwnership(id, data)
	if err != nil {
		return NetworkOwnership{}, false, err
	}
	return ownership, true, nil
}

func (s *Store) clearNetworkOwnershipUnlocked(id string) error {
	if err := validateID(id); err != nil {
		return err
	}
	return removeStateFileDurable(s.ctrDir, networkOwnershipPath(s.ctrDir, id), "network ownership")
}

func (s *Store) GetNetworkOwnership(id string) (NetworkOwnership, bool, error) {
	if s == nil {
		return NetworkOwnership{}, false, fmt.Errorf("state store is nil")
	}
	if err := validateID(id); err != nil {
		return NetworkOwnership{}, false, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.lockFile == nil {
		return NetworkOwnership{}, false, ErrStoreClosed
	}
	return s.readNetworkOwnershipUnlocked(id)
}

// MarkNetworkOwnedIfIdentity records cleanup intent before host-network
// mutation. Repeating the same ownership is idempotent; another generation's
// pending ownership must be cleaned before new resources can be installed.
func (s *Store) MarkNetworkOwnedIfIdentity(id string, ownership NetworkOwnership) error {
	if s == nil {
		return fmt.Errorf("state store is nil")
	}
	if err := validateID(id); err != nil {
		return err
	}
	if err := validateNetworkOwnership(ownership); err != nil {
		return err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	c, err := s.getUnlocked(id)
	if err != nil {
		return err
	}
	if c.Status != StatusRunning || c.PID != ownership.PID || c.PIDStartTime != ownership.PIDStartTime {
		return fmt.Errorf("container %s is not bound to process %d/%d while recording network ownership", id, ownership.PID, ownership.PIDStartTime)
	}

	existing, ok, err := s.readNetworkOwnershipUnlocked(id)
	if err != nil {
		return err
	}
	if ok {
		if networkOwnershipEqual(existing, ownership) {
			return nil
		}
		return fmt.Errorf("container %s already has pending network ownership for %s (%d/%d)", id, existing.Owner, existing.PID, existing.PIDStartTime)
	}
	return s.writeNetworkOwnershipUnlocked(id, ownership)
}

func networkOwnershipEqual(a, b NetworkOwnership) bool {
	if a.Owner != b.Owner || a.PID != b.PID || a.PIDStartTime != b.PIDStartTime || a.VethHost != b.VethHost || len(a.Mappings) != len(b.Mappings) {
		return false
	}
	for i := range a.Mappings {
		if a.Mappings[i] != b.Mappings[i] {
			return false
		}
	}
	return true
}

// ClearNetworkOwnershipIfMatch clears only one exact ownership record after
// cleanup. While the public state still says running, it is allowed only for
// the same persisted process identity; a stale actor cannot clear ownership
// after another generation has replaced the record.
func (s *Store) ClearNetworkOwnershipIfMatch(id string, ownership NetworkOwnership) (bool, error) {
	if s == nil {
		return false, fmt.Errorf("state store is nil")
	}
	if err := validateID(id); err != nil {
		return false, err
	}
	if err := validateNetworkOwnership(ownership); err != nil {
		return false, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return false, err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	existing, ok, err := s.readNetworkOwnershipUnlocked(id)
	if err != nil {
		return false, err
	}
	if !ok || !networkOwnershipEqual(existing, ownership) {
		return false, nil
	}

	c, err := s.getUnlocked(id)
	if err != nil {
		return false, err
	}
	if c.Status == StatusRunning && (c.PID != ownership.PID || c.PIDStartTime != ownership.PIDStartTime) {
		return false, fmt.Errorf("refusing to clear network ownership for stale generation %d/%d while container %s runs as %d/%d", ownership.PID, ownership.PIDStartTime, id, c.PID, c.PIDStartTime)
	}
	if err := s.clearNetworkOwnershipUnlocked(id); err != nil {
		return false, err
	}
	return true, nil
}
