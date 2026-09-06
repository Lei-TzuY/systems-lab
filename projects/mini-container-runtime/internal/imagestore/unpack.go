package imagestore

import (
	"fmt"
	"os"
)

// VerifyRootFSTree checks if rootfsPath exists as a valid directory.
func VerifyRootFSTree(rootfsPath string) (bool, error) {
	if rootfsPath == "" {
		return false, fmt.Errorf("rootfs path is empty")
	}

	fi, err := os.Stat(rootfsPath)
	if err != nil {
		if os.IsNotExist(err) {
			return false, fmt.Errorf("rootfs directory %s does not exist", rootfsPath)
		}
		return false, fmt.Errorf("stat rootfs: %w", err)
	}

	if !fi.IsDir() {
		return false, fmt.Errorf("%s is not a directory", rootfsPath)
	}

	return true, nil
}
