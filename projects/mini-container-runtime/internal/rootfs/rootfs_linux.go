//go:build linux

// internal/rootfs/rootfs_linux.go
//
// Filesystem Isolation — pivot_root
// ───────────────────────────────
// Container filesystem isolation means the container process sees a
// completely different directory tree than the host. This runtime requires
// pivot_root(2): unlike chroot(2), pivot_root lets us detach the old root mount
// entirely instead of silently retaining a weaker filesystem-isolation model.
//
// pivot_root algorithm
// ────────────────────
//  1. Bind-mount newRoot on itself → creates a mount-point entry in the
//     kernel's mount table, which pivot_root requires.
//  2. mkdir newRoot/.pivot_old  → parking spot for the old root.
//  3. pivot_root(newRoot, newRoot/.pivot_old)
//       → kernel swaps "/" to newRoot, old "/" is at /.pivot_old.
//  4. chdir "/"  → update CWD to the new root.
//  5. umount2("/.pivot_old", MNT_DETACH)
//       → lazy-unmount: detached immediately but stays accessible to
//         existing open file descriptors until they're all closed.
//  6. rmdir "/.pivot_old"  → clean up.

package rootfs

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"syscall"
)

type pivotRootFunc func(newRoot string, debug bool) error
type deviceSetupFunc func(newRoot string, debug bool) error

type pivotRootOps struct {
	mount   func(source, target, fstype string, flags uintptr, data string) error
	mkdir   func(path string, mode os.FileMode) error
	pivot   func(newRoot, putOld string) error
	chdir   func(path string) error
	unmount func(target string, flags int) error
	remove  func(path string) error
}

func defaultPivotRootOps() pivotRootOps {
	return pivotRootOps{
		mount:   syscall.Mount,
		mkdir:   os.Mkdir,
		pivot:   syscall.PivotRoot,
		chdir:   syscall.Chdir,
		unmount: syscall.Unmount,
		remove:  os.Remove,
	}
}

// Isolate changes the root filesystem of the current process to newRoot.
// It must be called after the process has entered a new mount namespace
// (CLONE_NEWNS), otherwise the bind-mount in step 1 would affect the host.
//
// Filesystem isolation is fail-closed: a private /dev must be established and
// pivot_root must succeed. We never exec the payload with the recursively
// inherited host /dev or downgrade to chroot.
func Isolate(newRoot string, debug bool) error {
	return isolateWithDeviceSetup(newRoot, debug, preparePrivateDevices, pivotRoot)
}

func isolateWithDeviceSetup(newRoot string, debug bool, setup deviceSetupFunc, pivot pivotRootFunc) error {
	if setup == nil {
		return fmt.Errorf("private /dev isolation function is nil")
	}
	if err := setup(newRoot, debug); err != nil {
		return fmt.Errorf("private /dev isolation required: %w", err)
	}
	return isolateWithPivot(newRoot, debug, pivot)
}

func isolateWithPivot(newRoot string, debug bool, pivot pivotRootFunc) error {
	if pivot == nil {
		return fmt.Errorf("pivot_root isolation function is nil")
	}
	if err := pivot(newRoot, debug); err != nil {
		return fmt.Errorf("pivot_root isolation required: %w", err)
	}
	return nil
}

// pivotRoot implements filesystem isolation using the pivot_root(2) syscall.
// This is the technique used by runc and Docker's default runtime.
func pivotRoot(newRoot string, debug bool) error {
	return pivotRootWithOps(newRoot, debug, defaultPivotRootOps())
}

func pivotRootWithOps(newRoot string, debug bool, ops pivotRootOps) (resultErr error) {
	if ops.mount == nil || ops.mkdir == nil || ops.pivot == nil || ops.chdir == nil || ops.unmount == nil || ops.remove == nil {
		return fmt.Errorf("pivot_root operations are incomplete")
	}

	// Step 1: bind-mount newRoot onto itself.
	//
	// pivot_root(2) requires newRoot to already be a mount point. A plain
	// directory is NOT a mount point. Bind-mounting the directory onto itself
	// promotes it to a mount point without changing its contents.
	//
	// MS_REC propagates the bind to all sub-mounts (e.g., /proc and /sys
	// that ContainerInit mounted inside newRoot before calling us).
	if err := ops.mount(newRoot, newRoot, "", syscall.MS_BIND|syscall.MS_REC, ""); err != nil {
		return fmt.Errorf("bind-mount rootfs onto itself: %w", err)
	}
	rootBindMounted := true
	pivoted := false
	pivotDirOwned := false
	oldRootDetached := false
	pivotDir := filepath.Join(newRoot, ".pivot_old")

	// Roll back every resource this generation can prove it created. Before
	// pivot_root, the self-bind is namespace-local and can be detached directly.
	// After pivot_root, the runtime-owned put_old directory is visible at
	// /.pivot_old and must not be left in a shared rootfs when init aborts.
	defer func() {
		if resultErr == nil {
			return
		}

		if pivoted {
			cleanupPath := "/.pivot_old"
			if !oldRootDetached {
				if err := ops.unmount(cleanupPath, syscall.MNT_DETACH); err != nil {
					resultErr = errors.Join(resultErr, fmt.Errorf("rollback detach old root: %w", err))
				} else {
					oldRootDetached = true
				}
			}
			if pivotDirOwned && oldRootDetached {
				if err := ops.remove(cleanupPath); err != nil {
					resultErr = errors.Join(resultErr, fmt.Errorf("rollback remove %s: %w", cleanupPath, err))
				} else {
					pivotDirOwned = false
				}
			}
			return
		}

		if pivotDirOwned {
			if err := ops.remove(pivotDir); err != nil {
				resultErr = errors.Join(resultErr, fmt.Errorf("rollback remove %s: %w", pivotDir, err))
			} else {
				pivotDirOwned = false
			}
		}
		if rootBindMounted {
			if err := ops.unmount(newRoot, syscall.MNT_DETACH); err != nil {
				resultErr = errors.Join(resultErr, fmt.Errorf("rollback rootfs bind mount %s: %w", newRoot, err))
			} else {
				rootBindMounted = false
			}
		}
	}()

	// Step 2: create an exclusively owned temporary directory inside newRoot for
	// the old root. Refuse a pre-existing path instead of borrowing and later
	// deleting a directory that belongs to the image/user.
	if err := ops.mkdir(pivotDir, 0o700); err != nil {
		return fmt.Errorf("mkdir .pivot_old: %w", err)
	}
	pivotDirOwned = true

	// Step 3: invoke pivot_root.
	//   newRoot  → becomes the new "/"
	//   pivotDir → old "/" is bind-mounted here (visible as /.pivot_old)
	if err := ops.pivot(newRoot, pivotDir); err != nil {
		return fmt.Errorf("pivot_root(%s, %s): %w", newRoot, pivotDir, err)
	}
	pivoted = true
	rootBindMounted = false

	// Step 4: update our CWD to the new root.
	// After pivot_root the process's CWD is still conceptually in the old
	// root (the kernel hasn't changed it automatically).
	if err := ops.chdir("/"); err != nil {
		return fmt.Errorf("chdir /: %w", err)
	}

	// Step 5: unmount the old root with MNT_DETACH (lazy unmount).
	// MNT_DETACH detaches the mount immediately from the filesystem tree
	// while keeping it accessible to any process that already has it open.
	if err := ops.unmount("/.pivot_old", syscall.MNT_DETACH); err != nil {
		return fmt.Errorf("unmount old root: %w", err)
	}
	oldRootDetached = true

	// Step 6: remove the now-empty runtime-owned directory. Failure is a runtime
	// teardown failure: silently continuing would persist a runtime artifact in
	// a shared rootfs.
	if err := ops.remove("/.pivot_old"); err != nil {
		return fmt.Errorf("remove /.pivot_old: %w", err)
	}
	pivotDirOwned = false

	if debug {
		fmt.Println("[init] pivot_root complete")
	}
	return nil
}
