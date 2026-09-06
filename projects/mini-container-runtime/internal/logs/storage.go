package logs

import (
	"path/filepath"

	"minicontainer/internal/state"
)

func managedLogStateDir() string {
	return filepath.Clean(state.DefaultDir())
}

func managedLogDir() string {
	return filepath.Join(managedLogStateDir(), "containers")
}

func isManagedLogPath(path string) bool {
	return filepath.Clean(filepath.Dir(path)) == managedLogDir()
}
