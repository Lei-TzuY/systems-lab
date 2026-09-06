//go:build linux

package dns

import (
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"os"

	"golang.org/x/sys/unix"
)

func readDNSRegistryFileAt(dirFD int, name, networkName string) ([]byte, bool, error) {
	fd, err := unix.Openat(dirFD, name, unix.O_RDONLY|unix.O_CLOEXEC|unix.O_NOFOLLOW|unix.O_NONBLOCK, 0)
	if err != nil {
		if err == unix.ENOENT {
			return nil, false, nil
		}
		if err == unix.ELOOP {
			return nil, false, fmt.Errorf("DNS registry %q must be a regular file: %w", networkName, err)
		}
		return nil, false, fmt.Errorf("open DNS registry %q: %w", networkName, err)
	}
	file := os.NewFile(uintptr(fd), name)
	if file == nil {
		_ = unix.Close(fd)
		return nil, false, fmt.Errorf("open DNS registry %q: invalid file descriptor", networkName)
	}
	defer file.Close()

	var st unix.Stat_t
	if err := unix.Fstat(fd, &st); err != nil {
		return nil, false, fmt.Errorf("inspect DNS registry %q: %w", networkName, err)
	}
	if st.Mode&unix.S_IFMT != unix.S_IFREG {
		return nil, false, fmt.Errorf("DNS registry %q must be a regular file", networkName)
	}
	if st.Nlink != 1 {
		return nil, false, fmt.Errorf("DNS registry %q must be a single-linked regular file", networkName)
	}
	data, err := readDNSRegistryContents(file, st.Size, networkName)
	if err != nil {
		return nil, false, err
	}
	return data, true, nil
}

func dnsTempName(networkName string) (string, error) {
	if err := validateDNSNetworkFilenameLength(networkName); err != nil {
		return "", err
	}
	var nonce [8]byte
	if _, err := rand.Read(nonce[:]); err != nil {
		return "", err
	}
	return "." + networkName + ".json.tmp-" + hex.EncodeToString(nonce[:]), nil
}

func saveDNSRegistryFileAtomicAt(dirFD int, name, networkName string, data []byte) error {
	if err := validateDNSNetworkFilenameLengthAt(dirFD, networkName); err != nil {
		return err
	}
	var tmpName string
	var fd int
	var err error
	for attempt := 0; attempt < 8; attempt++ {
		tmpName, err = dnsTempName(networkName)
		if err != nil {
			return fmt.Errorf("name DNS registry temp file %q: %w", networkName, err)
		}
		fd, err = unix.Openat(dirFD, tmpName, unix.O_CREAT|unix.O_EXCL|unix.O_WRONLY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0o600)
		if err == nil {
			break
		}
		if err != unix.EEXIST {
			return fmt.Errorf("create DNS registry temp file %q: %w", networkName, err)
		}
	}
	if err != nil {
		return fmt.Errorf("create DNS registry temp file %q: %w", networkName, err)
	}
	published := false
	defer func() {
		_ = unix.Close(fd)
		if !published {
			_ = unix.Unlinkat(dirFD, tmpName, 0)
		}
	}()

	file := os.NewFile(uintptr(fd), tmpName)
	if file == nil {
		return fmt.Errorf("create DNS registry temp file %q: invalid file descriptor", networkName)
	}
	written := 0
	for written < len(data) {
		n, writeErr := file.Write(data[written:])
		if writeErr != nil {
			return fmt.Errorf("write DNS registry temp file %q: %w", networkName, writeErr)
		}
		if n == 0 {
			return fmt.Errorf("write DNS registry temp file %q: short write %d/%d", networkName, written, len(data))
		}
		written += n
	}
	if err := file.Sync(); err != nil {
		return fmt.Errorf("sync DNS registry temp file %q: %w", networkName, err)
	}
	if err := file.Close(); err != nil {
		return fmt.Errorf("close DNS registry temp file %q: %w", networkName, err)
	}
	fd = -1
	if err := unix.Renameat(dirFD, tmpName, dirFD, name); err != nil {
		return fmt.Errorf("publish DNS registry %q: %w", networkName, err)
	}
	published = true
	if err := unix.Fsync(dirFD); err != nil {
		return fmt.Errorf("sync DNS registry directory: %w", err)
	}
	return nil
}
