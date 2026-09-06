//go:build linux

package container

import (
	"errors"
	"fmt"
	"os"
	"strconv"
	"syscall"

	"golang.org/x/sys/unix"
	"minicontainer/internal/dns"
)

const (
	runtimeHostsFD       uintptr = 5
	bridgeDNSNetworkName        = "default"
	runtimeHostsPrefix          = "minicontainer-hosts-"
)

type runtimeHostsCreateTemp func(dir, pattern string) (*os.File, error)
type runtimeHostsRemove func(path string) error
type runtimeHostsMount func(source, target, fstype string, flags uintptr, data string) error

// createRuntimeHostsFile snapshots the current bridge DNS registry into an
// anonymous regular file. The pathname is unlinked before the child starts;
// ownership therefore lives only in the parent/child file descriptors and the
// eventual bind mount, with no host pathname that can survive a parent crash.
func createRuntimeHostsFile(enabled bool) (*os.File, error) {
	if !enabled {
		return nil, nil
	}
	content, err := dns.GenerateHostsContentChecked(bridgeDNSNetworkName)
	if err != nil {
		return nil, &runtimeSetupError{err: fmt.Errorf("read bridge DNS registry: %w", err)}
	}
	return createRuntimeHostsFileWith(content, os.CreateTemp, os.Remove)
}

func createRuntimeHostsFileWith(content string, createTemp runtimeHostsCreateTemp, remove runtimeHostsRemove) (*os.File, error) {
	if createTemp == nil || remove == nil {
		return nil, &runtimeSetupError{err: fmt.Errorf("prepare runtime hosts file: operation is nil")}
	}
	file, err := createTemp("", runtimeHostsPrefix+"*")
	if err != nil {
		return nil, &runtimeSetupError{err: fmt.Errorf("create runtime hosts file: %w", err)}
	}
	if file == nil {
		return nil, &runtimeSetupError{err: fmt.Errorf("create runtime hosts file: nil file returned")}
	}
	path := file.Name()
	cleanup := func(base error) error {
		var cleanupErrs []error
		if err := file.Close(); err != nil && !errors.Is(err, os.ErrClosed) {
			cleanupErrs = append(cleanupErrs, fmt.Errorf("close runtime hosts file: %w", err))
		}
		if path != "" {
			if err := remove(path); err != nil && !os.IsNotExist(err) {
				cleanupErrs = append(cleanupErrs, fmt.Errorf("remove runtime hosts file %q: %w", path, err))
			}
		}
		return errors.Join(base, errors.Join(cleanupErrs...))
	}

	if _, err := file.WriteString(content); err != nil {
		return nil, &runtimeSetupError{err: cleanup(fmt.Errorf("write runtime hosts file: %w", err))}
	}
	if err := file.Chmod(0o644); err != nil {
		return nil, &runtimeSetupError{err: cleanup(fmt.Errorf("chmod runtime hosts file: %w", err))}
	}
	if err := remove(path); err != nil {
		return nil, &runtimeSetupError{err: cleanup(fmt.Errorf("unlink runtime hosts file %q: %w", path, err))}
	}
	return file, nil
}

func runtimeHostsFileFromFD(enabled bool) (*os.File, error) {
	if !enabled {
		return nil, nil
	}
	file := os.NewFile(runtimeHostsFD, "runtime-hosts")
	if file == nil {
		return nil, fmt.Errorf("runtime hosts fd %d is unavailable", runtimeHostsFD)
	}
	if err := validateRuntimeHostsFile(file); err != nil {
		_ = file.Close()
		return nil, err
	}
	return file, nil
}

func validateRuntimeHostsFile(file *os.File) error {
	if file == nil {
		return fmt.Errorf("runtime hosts file is nil")
	}
	var st unix.Stat_t
	if err := unix.Fstat(int(file.Fd()), &st); err != nil {
		return fmt.Errorf("inspect runtime hosts fd: %w", err)
	}
	if st.Mode&unix.S_IFMT != unix.S_IFREG {
		return fmt.Errorf("runtime hosts fd is not a regular file")
	}
	if st.Nlink != 0 {
		return fmt.Errorf("runtime hosts fd still has %d host link(s)", st.Nlink)
	}
	return nil
}

func mountRuntimeHostsFile(source *os.File, rootfs string, debug bool) error {
	return mountRuntimeHostsFileWith(source, rootfs, debug, syscall.Mount)
}

func mountRuntimeHostsFileWith(source *os.File, rootfs string, debug bool, mount runtimeHostsMount) error {
	if source == nil {
		return nil
	}
	if mount == nil {
		return fmt.Errorf("runtime hosts mount operation is nil")
	}
	targetFD, err := openRuntimeHostsTarget(rootfs)
	if err != nil {
		return err
	}
	defer unix.Close(targetFD)

	sourcePath := "/proc/self/fd/" + strconv.Itoa(int(source.Fd()))
	targetPath := volumeTargetFDPath(targetFD)
	if err := mount(sourcePath, targetPath, "", syscall.MS_BIND, ""); err != nil {
		return fmt.Errorf("bind runtime hosts file: %w", err)
	}
	if debug {
		fmt.Println("[init] mounted ephemeral /etc/hosts")
	}
	return nil
}

func openRuntimeHostsTarget(rootfs string) (int, error) {
	if rootfs == "" {
		return -1, fmt.Errorf("runtime hosts rootfs is empty")
	}
	rootFD, err := unix.Open(rootfs, unix.O_PATH|unix.O_DIRECTORY|unix.O_CLOEXEC, 0)
	if err != nil {
		return -1, fmt.Errorf("open rootfs for runtime hosts: %w", err)
	}
	defer unix.Close(rootFD)

	how := &unix.OpenHow{
		Flags: uint64(unix.O_PATH | unix.O_CLOEXEC),
		Resolve: uint64(
			unix.RESOLVE_IN_ROOT |
				unix.RESOLVE_NO_MAGICLINKS |
				unix.RESOLVE_NO_SYMLINKS,
		),
	}
	fd, err := unix.Openat2(rootFD, "etc/hosts", how)
	if errors.Is(err, unix.ENOSYS) {
		return openRuntimeHostsTargetFallback(rootFD)
	}
	if err != nil {
		return -1, fmt.Errorf("resolve existing /etc/hosts without symlinks: %w", err)
	}
	if err := requireRegularHostsTarget(fd); err != nil {
		_ = unix.Close(fd)
		return -1, err
	}
	return fd, nil
}

func openRuntimeHostsTargetFallback(rootFD int) (int, error) {
	etcFD, err := unix.Openat(rootFD, "etc", unix.O_PATH|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return -1, fmt.Errorf("open /etc without symlinks: %w", err)
	}
	defer unix.Close(etcFD)

	fd, err := unix.Openat(etcFD, "hosts", unix.O_PATH|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return -1, fmt.Errorf("open existing /etc/hosts without symlinks: %w", err)
	}
	if err := requireRegularHostsTarget(fd); err != nil {
		_ = unix.Close(fd)
		return -1, err
	}
	return fd, nil
}

func requireRegularHostsTarget(fd int) error {
	var st unix.Stat_t
	if err := unix.Fstat(fd, &st); err != nil {
		return fmt.Errorf("inspect /etc/hosts target: %w", err)
	}
	if st.Mode&unix.S_IFMT != unix.S_IFREG {
		return fmt.Errorf("existing /etc/hosts target is not a regular file")
	}
	return nil
}
