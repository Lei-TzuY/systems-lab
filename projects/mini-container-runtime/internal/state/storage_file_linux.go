//go:build linux

package state

import (
	"fmt"
	"os"

	"golang.org/x/sys/unix"
)

// inspectRegularStateFD validates the inode already pinned by fd. A link count
// of zero is valid: atomic replacement can detach an inode from the namespace
// after open(2), while the descriptor still safely refers to that pinned state
// generation. More than one link is rejected because another pathname could
// alias the same authoritative state inode.
func inspectRegularStateFD(fd int, path, label string) (unix.Stat_t, error) {
	var st unix.Stat_t
	if err := unix.Fstat(fd, &st); err != nil {
		return st, fmt.Errorf("inspect %s %q: %w", label, path, err)
	}
	if st.Mode&unix.S_IFMT != unix.S_IFREG {
		return st, fmt.Errorf("%s %q must be a regular file", label, path)
	}
	if st.Nlink > 1 {
		return st, fmt.Errorf("%s %q must be single-linked; hard-link aliases detected", label, path)
	}
	return st, nil
}

// readRegularStateFile opens the state file without following a final symlink,
// then validates and tightens permissions on the already-open descriptor. This
// avoids the Lstat/open TOCTOU window where a pathname could be swapped to a
// symlink between validation and reading.
func readRegularStateFile(path, label string) ([]byte, error) {
	fd, err := unix.Open(path, unix.O_RDONLY|unix.O_CLOEXEC|unix.O_NOFOLLOW|unix.O_NONBLOCK, 0)
	if err != nil {
		return nil, err
	}
	file := os.NewFile(uintptr(fd), path)
	if file == nil {
		_ = unix.Close(fd)
		return nil, fmt.Errorf("wrap %s fd", label)
	}
	defer file.Close()

	st, err := inspectRegularStateFD(fd, path, label)
	if err != nil {
		return nil, err
	}
	if err := unix.Fchmod(fd, 0o600); err != nil {
		return nil, fmt.Errorf("secure %s permissions: %w", label, err)
	}
	return readBoundedStateFile(file, st.Size, label)
}
