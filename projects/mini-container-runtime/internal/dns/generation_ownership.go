package dns

import (
	"fmt"
	"strings"
)

// BindHostRegistrationGeneration durably upgrades a registrar-owned DNS entry
// to exact child-generation ownership after bridge admission succeeds. Runtime
// admission reservations become visible to peers in the same atomic registry
// update that binds the exact child generation.
func BindHostRegistrationGeneration(networkName, containerID string, pid int, pidStartTime uint64) error {
	if err := validateNetworkName(networkName); err != nil {
		return err
	}
	if strings.TrimSpace(containerID) == "" {
		return fmt.Errorf("container ID cannot be empty")
	}
	if pid <= 0 || pidStartTime == 0 {
		return fmt.Errorf("invalid DNS child process identity %d/%d", pid, pidStartTime)
	}
	owner, err := currentRegistrarIdentity()
	if err != nil {
		return err
	}

	dnsMu.Lock()
	defer dnsMu.Unlock()
	dir, err := ensureDNSDir()
	if err != nil {
		return err
	}
	return withDNSNetworkLock(dir, networkName, func(dirFD int) error {
		netName := networkName + ".json"
		entries, exists, err := loadEntriesCheckedAt(dirFD, netName, networkName)
		if err != nil {
			return err
		}
		if !exists {
			return fmt.Errorf("DNS registration for container %q does not exist", containerID)
		}
		for i, entry := range entries {
			if entry.ContainerID != containerID {
				continue
			}
			if entry.OwnerPID != owner.PID || entry.OwnerStartTime != owner.StartTime {
				return fmt.Errorf(
					"DNS registration for container %q is owned by registrar %d/%d, not current registrar %d/%d",
					containerID,
					entry.OwnerPID,
					entry.OwnerStartTime,
					owner.PID,
					owner.StartTime,
				)
			}
			if !entry.GenerationAware {
				return fmt.Errorf("DNS registration for container %q is not generation-aware", containerID)
			}
			if entry.GenerationPID == pid && entry.GenerationStartTime == pidStartTime && !entry.AdmissionPending {
				return nil
			}
			if (entry.GenerationPID != 0 || entry.GenerationStartTime != 0) &&
				(entry.GenerationPID != pid || entry.GenerationStartTime != pidStartTime) {
				return fmt.Errorf(
					"DNS registration for container %q is already bound to child generation %d/%d",
					containerID,
					entry.GenerationPID,
					entry.GenerationStartTime,
				)
			}
			updated := append([]HostEntry(nil), entries...)
			updated[i].GenerationPID = pid
			updated[i].GenerationStartTime = pidStartTime
			updated[i].AdmissionPending = false
			return saveEntriesAtomicAt(dirFD, netName, networkName, updated)
		}
		return fmt.Errorf("DNS registration for container %q does not exist", containerID)
	})
}
