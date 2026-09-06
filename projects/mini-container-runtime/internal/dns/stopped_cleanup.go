package dns

import (
	"fmt"
	"os"
	"strings"
)

type childGenerationIdentity struct {
	PID       int
	StartTime uint64
}

func (g childGenerationIdentity) valid() bool {
	return g.PID > 0 && g.StartTime != 0
}

// CleanupStoppedHostRegistrationGeneration removes DNS state that can be
// retired under authority for one exact stopped child PID/start-time generation.
// Modern generation-aware entries are treated as a CAS token: an entry bound to
// a different child generation, or still unbound during a newer admission, is
// never consumed merely because its registrar matches or is stale.
func CleanupStoppedHostRegistrationGeneration(networkName, containerID string, generationPID int, generationStartTime uint64) error {
	generation := childGenerationIdentity{PID: generationPID, StartTime: generationStartTime}
	return cleanupStoppedHostRegistrationWithGenerationPolicy(networkName, containerID, generation)
}

func validateStoppedGenerationCleanupInputs(networkName, containerID string) error {
	if err := validateNetworkName(networkName); err != nil {
		return err
	}
	if strings.TrimSpace(containerID) == "" {
		return fmt.Errorf("container ID cannot be empty")
	}
	return nil
}

func withStoppedDNSRegistry(networkName string, mutate func([]HostEntry) ([]HostEntry, bool, error)) error {
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
		updated, changed, err := mutate(entries)
		if err != nil || !changed {
			return err
		}
		return saveEntriesAtomicAt(dirFD, netName, networkName, updated)
	})
}

func cleanupStoppedHostRegistrationWithGenerationPolicy(
	networkName, containerID string,
	generation childGenerationIdentity,
) error {
	if err := validateStoppedGenerationCleanupInputs(networkName, containerID); err != nil {
		return err
	}
	if !generation.valid() {
		return fmt.Errorf("invalid DNS child process identity %d/%d", generation.PID, generation.StartTime)
	}

	return withStoppedDNSRegistry(networkName, func(entries []HostEntry) ([]HostEntry, bool, error) {
		updated := make([]HostEntry, 0, len(entries))
		removed := false
		for _, entry := range entries {
			if entry.ContainerID != containerID {
				updated = append(updated, entry)
				continue
			}

			if entry.GenerationAware {
				if entry.GenerationPID == 0 && entry.GenerationStartTime == 0 {
					updated = append(updated, entry)
					continue
				}
				if entry.GenerationPID == generation.PID && entry.GenerationStartTime == generation.StartTime {
					removed = true
					continue
				}
				updated = append(updated, entry)
				continue
			}

			removed = true
		}
		return updated, removed, nil
	})
}
