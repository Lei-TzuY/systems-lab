//go:build linux

package image

import (
	"errors"
	"fmt"
	"os"

	"golang.org/x/sys/unix"
)

type fchownFunc func(fd, uid, gid int) error

func restoreOwnershipWith(fd, uid, gid, euid int, chown fchownFunc) error {
	if uid < 0 || gid < 0 {
		return fmt.Errorf("invalid negative tar ownership %d:%d", uid, gid)
	}
	if chown == nil {
		return fmt.Errorf("fchown operation is nil")
	}
	if err := chown(fd, uid, gid); err != nil {
		// Rootless extraction cannot represent arbitrary archive ownership.
		// Preserve usability by leaving the inode owned by the caller only for
		// the expected unprivileged EPERM case; privileged failures remain fatal.
		if errors.Is(err, unix.EPERM) && euid != 0 {
			return nil
		}
		return err
	}
	return nil
}

func restoreOwnershipFD(fd int, target string, uid, gid int) error {
	if err := restoreOwnershipWith(fd, uid, gid, os.Geteuid(), unix.Fchown); err != nil {
		return fmt.Errorf("restore ownership %d:%d on %s: %w", uid, gid, target, err)
	}
	return nil
}
