package state

import (
	"fmt"
	"os"
	"strings"
)

func ensurePrivateStateDir(path, label string) error {
	if strings.TrimSpace(path) == "" {
		return fmt.Errorf("%s directory cannot be empty", label)
	}
	if err := os.MkdirAll(path, 0o700); err != nil {
		return fmt.Errorf("create %s directory: %w", label, err)
	}
	info, err := os.Lstat(path)
	if err != nil {
		return fmt.Errorf("inspect %s directory: %w", label, err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return fmt.Errorf("%s path %q must be a real directory", label, path)
	}
	if err := os.Chmod(path, 0o700); err != nil {
		return fmt.Errorf("secure %s directory permissions: %w", label, err)
	}
	return nil
}

func syncStateDirectory(dir, label string) error {
	f, err := os.Open(dir)
	if err != nil {
		return fmt.Errorf("open %s directory for fsync: %w", label, err)
	}
	defer f.Close()
	if err := f.Sync(); err != nil {
		return fmt.Errorf("fsync %s directory: %w", label, err)
	}
	return nil
}

func removeStateFileDurable(dir, path, label string) error {
	if err := os.Remove(path); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("remove %s: %w", label, err)
	}
	if label == "container state" {
		if err := removeExitedIdentityForContainerState(path); err != nil {
			return err
		}
	}
	// Sync even when the file is already absent. This lets a retry repair the
	// durability of a previous unlink whose directory fsync reported an error.
	return syncStateDirectory(dir, label)
}
