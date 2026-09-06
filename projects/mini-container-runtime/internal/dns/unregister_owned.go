package dns

import (
	"fmt"
	"os"
	"strings"
)

// UnregisterHostOwned removes a DNS entry only when it is still owned by this
// exact registrar process generation. A stale runtime finalizer must never
// delete a replacement entry for the same container ID that a newer registrar
// has already published.
func UnregisterHostOwned(networkName, containerID string) error {
	owner, err := currentRegistrarIdentity()
	if err != nil {
		return err
	}
	return unregisterHostIfOwnedBy(networkName, containerID, owner)
}

func unregisterHostIfOwnedBy(networkName, containerID string, owner registrarIdentity) error {
	if err := validateNetworkName(networkName); err != nil {
		return err
	}
	if strings.TrimSpace(containerID) == "" {
		return fmt.Errorf("container ID cannot be empty")
	}
	if owner.PID <= 0 || owner.StartTime == 0 {
		return fmt.Errorf("invalid DNS registrar process identity %d/%d", owner.PID, owner.StartTime)
	}

	dir := DefaultDNSDir()
	info, err := os.Lstat(dir)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return fmt.Errorf("inspect DNS registry directory %q: %w", dir, err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return fmt.Errorf("DNS registry path %q must be a real directory", dir)
	}

	dnsMu.Lock()
	defer dnsMu.Unlock()
	return withDNSNetworkLock(dir, networkName, func(dirFD int) error {
		netName := networkName + ".json"
		entries, exists, err := loadEntriesCheckedAt(dirFD, netName, networkName)
		if err != nil || !exists {
			return err
		}

		updated := make([]HostEntry, 0, len(entries))
		removed := false
		for _, entry := range entries {
			if entry.ContainerID != containerID {
				updated = append(updated, entry)
				continue
			}
			if entry.OwnerPID != owner.PID || entry.OwnerStartTime != owner.StartTime {
				updated = append(updated, entry)
				continue
			}
			removed = true
		}
		if !removed {
			return nil
		}
		return saveEntriesAtomicAt(dirFD, netName, networkName, updated)
	})
}
