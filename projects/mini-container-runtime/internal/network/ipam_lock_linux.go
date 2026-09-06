//go:build linux

package network

import (
	"errors"
	"fmt"
	"path/filepath"

	"golang.org/x/sys/unix"
)

// withIPAMNetworkLock serializes one network pool across independent minictl
// processes. The lock file lives beside the pool file so all processes that
// share an IPAM directory contend on the same kernel flock.
func withIPAMNetworkLock(dir, networkName string, fn func() error) error {
	if fn == nil {
		return fmt.Errorf("IPAM lock callback is nil")
	}
	lockPath := filepath.Join(dir, networkName+".lock")
	fd, err := unix.Open(lockPath, unix.O_CREAT|unix.O_RDWR|unix.O_CLOEXEC|unix.O_NOFOLLOW|unix.O_NONBLOCK, 0600)
	if err != nil {
		return fmt.Errorf("open IPAM lock %q: %w", networkName, err)
	}
	defer unix.Close(fd)

	var st unix.Stat_t
	if err := unix.Fstat(fd, &st); err != nil {
		return fmt.Errorf("inspect IPAM lock %q: %w", networkName, err)
	}
	if st.Mode&unix.S_IFMT != unix.S_IFREG {
		return fmt.Errorf("IPAM lock %q must be a regular file", networkName)
	}
	if err := unix.Fchmod(fd, 0600); err != nil {
		return fmt.Errorf("chmod IPAM lock %q: %w", networkName, err)
	}
	if err := unix.Flock(fd, unix.LOCK_EX); err != nil {
		return fmt.Errorf("lock IPAM network %q: %w", networkName, err)
	}

	callbackErr := fn()
	unlockErr := unix.Flock(fd, unix.LOCK_UN)
	if unlockErr != nil {
		unlockErr = fmt.Errorf("unlock IPAM network %q: %w", networkName, unlockErr)
	}
	return errors.Join(callbackErr, unlockErr)
}
