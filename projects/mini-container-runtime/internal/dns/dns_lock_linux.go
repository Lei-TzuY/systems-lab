//go:build linux

package dns

import (
	"errors"
	"fmt"

	"golang.org/x/sys/unix"
)

func verifyDNSDirPath(fd int, dir, networkName string) error {
	var held unix.Stat_t
	if err := unix.Fstat(fd, &held); err != nil {
		return fmt.Errorf("inspect DNS registry directory for %q: %w", networkName, err)
	}
	if held.Mode&unix.S_IFMT != unix.S_IFDIR {
		return fmt.Errorf("DNS registry path for %q must be a real directory", networkName)
	}

	var current unix.Stat_t
	if err := unix.Lstat(dir, &current); err != nil {
		return fmt.Errorf("verify DNS registry directory for %q path identity: %w", networkName, err)
	}
	if current.Mode&unix.S_IFMT != unix.S_IFDIR || current.Dev != held.Dev || current.Ino != held.Ino {
		return fmt.Errorf("DNS registry directory for %q changed while locked", networkName)
	}
	return nil
}

func verifyDNSLockPath(dirFD, fd int, lockName, networkName string) error {
	var held unix.Stat_t
	if err := unix.Fstat(fd, &held); err != nil {
		return fmt.Errorf("inspect DNS lock %q: %w", networkName, err)
	}
	if held.Mode&unix.S_IFMT != unix.S_IFREG {
		return fmt.Errorf("DNS lock %q must be a regular file", networkName)
	}
	if held.Nlink != 1 {
		return fmt.Errorf("DNS lock %q must have exactly one link, got %d", networkName, held.Nlink)
	}

	var current unix.Stat_t
	if err := unix.Fstatat(dirFD, lockName, &current, unix.AT_SYMLINK_NOFOLLOW); err != nil {
		return fmt.Errorf("verify DNS lock %q path identity: %w", networkName, err)
	}
	if current.Mode&unix.S_IFMT != unix.S_IFREG || current.Dev != held.Dev || current.Ino != held.Ino {
		return fmt.Errorf("DNS lock %q path changed while locked", networkName)
	}
	return nil
}

// withDNSNetworkLock serializes one DNS registry across independent minictl
// processes. The callback receives the already-verified registry-directory fd;
// registry reads and publication must stay relative to that descriptor so a
// pathname replacement cannot redirect I/O after lock acquisition.
func withDNSNetworkLock(dir, networkName string, fn func(dirFD int) error) error {
	if fn == nil {
		return fmt.Errorf("DNS lock callback is nil")
	}
	if err := validateDNSNetworkFilenameLength(networkName); err != nil {
		return err
	}

	dirFD, err := unix.Open(dir, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return fmt.Errorf("open DNS registry directory for %q: %w", networkName, err)
	}
	defer unix.Close(dirFD)
	if err := validateDNSNetworkFilenameLengthAt(dirFD, networkName); err != nil {
		return err
	}
	if err := unix.Fchmod(dirFD, 0o700); err != nil {
		return fmt.Errorf("chmod DNS registry directory for %q: %w", networkName, err)
	}
	if err := verifyDNSDirPath(dirFD, dir, networkName); err != nil {
		return err
	}

	lockName := networkName + ".lock"
	fd, err := unix.Openat(dirFD, lockName, unix.O_CREAT|unix.O_RDWR|unix.O_CLOEXEC|unix.O_NOFOLLOW|unix.O_NONBLOCK, 0o600)
	if err != nil {
		return fmt.Errorf("open DNS lock %q: %w", networkName, err)
	}
	defer unix.Close(fd)

	var st unix.Stat_t
	if err := unix.Fstat(fd, &st); err != nil {
		return fmt.Errorf("inspect DNS lock %q: %w", networkName, err)
	}
	if st.Mode&unix.S_IFMT != unix.S_IFREG {
		return fmt.Errorf("DNS lock %q must be a regular file", networkName)
	}
	if err := unix.Fchmod(fd, 0o600); err != nil {
		return fmt.Errorf("chmod DNS lock %q: %w", networkName, err)
	}
	if err := unix.Flock(fd, unix.LOCK_EX); err != nil {
		return fmt.Errorf("lock DNS network %q: %w", networkName, err)
	}
	if err := verifyDNSDirPath(dirFD, dir, networkName); err != nil {
		_ = unix.Flock(fd, unix.LOCK_UN)
		return err
	}
	if err := verifyDNSLockPath(dirFD, fd, lockName, networkName); err != nil {
		_ = unix.Flock(fd, unix.LOCK_UN)
		return err
	}

	callbackErr := fn(dirFD)
	dirIntegrityErr := verifyDNSDirPath(dirFD, dir, networkName)
	lockIntegrityErr := verifyDNSLockPath(dirFD, fd, lockName, networkName)
	unlockErr := unix.Flock(fd, unix.LOCK_UN)
	if unlockErr != nil {
		unlockErr = fmt.Errorf("unlock DNS network %q: %w", networkName, unlockErr)
	}
	return errors.Join(callbackErr, dirIntegrityErr, lockIntegrityErr, unlockErr)
}
