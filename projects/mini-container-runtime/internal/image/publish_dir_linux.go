//go:build linux

package image

import (
	"fmt"

	"golang.org/x/sys/unix"
)

func publishDirectoryNoReplace(staging, destination string) error {
	if err := unix.Renameat2(unix.AT_FDCWD, staging, unix.AT_FDCWD, destination, unix.RENAME_NOREPLACE); err != nil {
		return fmt.Errorf("publish staged directory %q as %q without replacement: %w", staging, destination, err)
	}
	return nil
}
