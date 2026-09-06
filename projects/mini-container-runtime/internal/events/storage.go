package events

import (
	"fmt"
	"os"
	"path/filepath"

	"minicontainer/internal/state"
)

func eventStateDir() string {
	return filepath.Clean(state.DefaultDir())
}

func isManagedEventLogPath(path string) bool {
	return filepath.Clean(path) == filepath.Join(eventStateDir(), "events.log")
}

// validateEventStagingStorage checks the managed event directory without
// creating events.log. Start events are staged before runtime readiness, but
// staging must retain the same fail-closed boundary behavior as an immediate
// append when the configured state root is a symlink or non-directory.
// The later commit still uses the descriptor-based event-log opener, so a path
// replacement after this check cannot redirect the eventual write.
func validateEventStagingStorage() error {
	dir := eventStateDir()
	if err := os.MkdirAll(dir, 0o700); err != nil {
		return fmt.Errorf("create event state directory: %w", err)
	}
	info, err := os.Lstat(dir)
	if err != nil {
		return fmt.Errorf("inspect event state directory: %w", err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return fmt.Errorf("event state directory is not a real directory")
	}
	return nil
}
