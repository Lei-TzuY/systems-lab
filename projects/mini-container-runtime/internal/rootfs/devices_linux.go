//go:build linux

package rootfs

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"syscall"
)

const privateDevTmpfsSize = "65536k"

var safeDeviceNames = []string{
	"null",
	"zero",
	"full",
	"random",
	"urandom",
	"tty",
}

type deviceMountOps struct {
	ensureDir      func(string, os.FileMode) error
	unmount        func(string, int) error
	mount          func(string, string, string, uintptr, string) error
	createFile     func(string, os.FileMode) error
	symlink        func(string, string) error
	validateSource func(string) error
}

func defaultDeviceMountOps() deviceMountOps {
	return deviceMountOps{
		ensureDir: secureEnsureDeviceDir,
		unmount:   syscall.Unmount,
		mount:     syscall.Mount,
		createFile: func(path string, mode os.FileMode) error {
			f, err := os.OpenFile(path, os.O_CREATE|os.O_EXCL|os.O_WRONLY, mode)
			if err != nil {
				return err
			}
			return f.Close()
		},
		symlink: os.Symlink,
		validateSource: func(path string) error {
			info, err := os.Lstat(path)
			if err != nil {
				return err
			}
			mode := info.Mode()
			if mode&os.ModeSymlink != 0 || mode&os.ModeDevice == 0 || mode&os.ModeCharDevice == 0 {
				return fmt.Errorf("%s is not a direct character device", path)
			}
			return nil
		},
	}
}

func secureEnsureDeviceDir(path string, mode os.FileMode) error {
	if err := os.MkdirAll(path, mode); err != nil {
		return err
	}
	info, err := os.Lstat(path)
	if err != nil {
		return err
	}
	if !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return fmt.Errorf("%s is not a real directory", path)
	}
	return nil
}

// preparePrivateDevices replaces any pre-pivot /dev mount with a private
// device filesystem. Only a small character-device allowlist is bind-mounted
// from the host; disks, GPUs, kmsg, fuse, and other host devices are not
// inherited by the container merely because they exist on the host.
func preparePrivateDevices(newRoot string, debug bool) error {
	return preparePrivateDevicesWithOps(newRoot, debug, defaultDeviceMountOps())
}

func preparePrivateDevicesWithOps(newRoot string, debug bool, ops deviceMountOps) error {
	if !filepath.IsAbs(newRoot) || filepath.Clean(newRoot) == "/" {
		return fmt.Errorf("invalid container root %q for private /dev", newRoot)
	}
	if ops.ensureDir == nil || ops.unmount == nil || ops.mount == nil || ops.createFile == nil || ops.symlink == nil || ops.validateSource == nil {
		return fmt.Errorf("private /dev mount operations are incomplete")
	}

	devPath := filepath.Join(newRoot, "dev")
	if err := ops.ensureDir(devPath, 0o755); err != nil {
		return fmt.Errorf("prepare /dev mountpoint: %w", err)
	}

	// ContainerInit historically bind-mounted the host /dev recursively before
	// pivot_root. Detach that view inside the private mount namespace before
	// constructing the allowlisted device filesystem. EINVAL means the path was
	// not a mountpoint, which is safe to continue from.
	if err := ops.unmount(devPath, syscall.MNT_DETACH); err != nil && !errors.Is(err, syscall.EINVAL) && !errors.Is(err, syscall.ENOENT) {
		return fmt.Errorf("detach inherited /dev: %w", err)
	}

	if err := ops.mount("tmpfs", devPath, "tmpfs", syscall.MS_NOSUID|syscall.MS_NOEXEC, "mode=0755,size="+privateDevTmpfsSize); err != nil {
		return fmt.Errorf("mount private /dev tmpfs: %w", err)
	}

	for _, name := range safeDeviceNames {
		source := filepath.Join("/dev", name)
		if err := ops.validateSource(source); err != nil {
			return fmt.Errorf("validate safe device %s: %w", source, err)
		}
		target := filepath.Join(devPath, name)
		if err := ops.createFile(target, 0o666); err != nil {
			return fmt.Errorf("create device target %s: %w", target, err)
		}
		if err := ops.mount(source, target, "", syscall.MS_BIND, ""); err != nil {
			return fmt.Errorf("bind safe device %s: %w", source, err)
		}
	}

	ptsPath := filepath.Join(devPath, "pts")
	if err := ops.ensureDir(ptsPath, 0o755); err != nil {
		return fmt.Errorf("prepare /dev/pts: %w", err)
	}
	if err := ops.mount("devpts", ptsPath, "devpts", syscall.MS_NOSUID|syscall.MS_NOEXEC, "newinstance,ptmxmode=0666,mode=0666"); err != nil {
		return fmt.Errorf("mount private devpts: %w", err)
	}
	if err := ops.symlink("pts/ptmx", filepath.Join(devPath, "ptmx")); err != nil {
		return fmt.Errorf("create /dev/ptmx: %w", err)
	}

	shmPath := filepath.Join(devPath, "shm")
	if err := ops.ensureDir(shmPath, 0o1777); err != nil {
		return fmt.Errorf("prepare /dev/shm: %w", err)
	}
	if err := ops.mount("tmpfs", shmPath, "tmpfs", syscall.MS_NOSUID|syscall.MS_NODEV|syscall.MS_NOEXEC, "mode=1777,size="+privateDevTmpfsSize); err != nil {
		return fmt.Errorf("mount private /dev/shm: %w", err)
	}

	for link, target := range map[string]string{
		"fd":     "/proc/self/fd",
		"stdin":  "/proc/self/fd/0",
		"stdout": "/proc/self/fd/1",
		"stderr": "/proc/self/fd/2",
	} {
		if err := ops.symlink(target, filepath.Join(devPath, link)); err != nil {
			return fmt.Errorf("create /dev/%s: %w", link, err)
		}
	}

	if debug {
		fmt.Println("[init] private /dev mounted with allowlisted devices")
	}
	return nil
}
