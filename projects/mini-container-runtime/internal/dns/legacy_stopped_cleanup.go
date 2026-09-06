package dns

import (
	"fmt"
	"strings"
)

type legacyHostEntryActiveProbe func(HostEntry) (bool, error)

// CleanupStoppedLegacyHostRegistrations retires every generation-unaware DNS
// record class for a historical stopped container in one registry transaction.
// Ownerless pre-ownership records are always migration debris once the caller
// has established an authoritative stopped lifecycle revision. Registrar-owned
// legacy records are removed only when their recorded owner is no longer active.
// Modern generation-aware records are never consumed by this compatibility path.
func CleanupStoppedLegacyHostRegistrations(networkName, containerID string) error {
	return cleanupStoppedLegacyHostRegistrationsWith(networkName, containerID, hostEntryOwnerActive)
}

func cleanupStoppedLegacyHostRegistrationsWith(
	networkName, containerID string,
	ownerActive legacyHostEntryActiveProbe,
) error {
	if err := validateNetworkName(networkName); err != nil {
		return err
	}
	if strings.TrimSpace(containerID) == "" {
		return fmt.Errorf("container ID cannot be empty")
	}
	if ownerActive == nil {
		return fmt.Errorf("DNS ownership activity probe is nil")
	}

	return withStoppedDNSRegistry(networkName, func(entries []HostEntry) ([]HostEntry, bool, error) {
		updated := make([]HostEntry, 0, len(entries))
		removed := false
		for _, entry := range entries {
			if entry.ContainerID != containerID || entry.GenerationAware {
				updated = append(updated, entry)
				continue
			}

			// Pre-ownership records cannot be recreated by current runtimes. The
			// caller's stopped-revision proof is therefore sufficient authority to
			// retire them without any process-liveness dependency.
			if entry.OwnerPID == 0 && entry.OwnerStartTime == 0 {
				removed = true
				continue
			}

			active, err := ownerActive(entry)
			if err != nil {
				return nil, false, fmt.Errorf("resolve legacy DNS ownership before stopped cleanup for container %s: %w", containerID, err)
			}
			if active {
				updated = append(updated, entry)
				continue
			}
			removed = true
		}
		return updated, removed, nil
	})
}
