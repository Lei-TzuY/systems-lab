//go:build !linux

package image

import (
	"fmt"
	"os"
)

func publishDirectoryNoReplace(staging, destination string) error {
	if _, err := os.Lstat(destination); err == nil {
		return fmt.Errorf("publish staged directory %q: destination %q already exists", staging, destination)
	} else if !os.IsNotExist(err) {
		return fmt.Errorf("inspect destination %q before publish: %w", destination, err)
	}
	if err := os.Rename(staging, destination); err != nil {
		return fmt.Errorf("publish staged directory %q as %q: %w", staging, destination, err)
	}
	return nil
}
