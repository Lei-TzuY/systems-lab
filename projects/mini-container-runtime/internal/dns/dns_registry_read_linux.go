//go:build linux

package dns

import (
	"fmt"
	"os"

	"golang.org/x/sys/unix"
)

func readDNSRegistryFile(path, networkName string) ([]byte, bool, error) {
	fd, err := unix.Open(path, unix.O_RDONLY|unix.O_CLOEXEC|unix.O_NOFOLLOW|unix.O_NONBLOCK, 0)
	if err != nil {
		if err == unix.ENOENT {
			return nil, false, nil
		}
		if err == unix.ELOOP {
			return nil, false, fmt.Errorf("DNS registry %q must be a regular file: %w", networkName, err)
		}
		return nil, false, fmt.Errorf("open DNS registry %q: %w", networkName, err)
	}
	file := os.NewFile(uintptr(fd), path)
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
