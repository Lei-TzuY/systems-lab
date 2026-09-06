//go:build linux

// internal/rootfs/overlay_linux.go
//
// OverlayFS — Copy-on-Write Layer Stacking
// ──────────────────────────────────────────
// OverlayFS (overlay2 in Docker terminology) is the default storage driver
// for Docker on modern Linux.  It merges multiple read-only directory trees
// (the image layers) into a single merged view, while directing all writes
// to a separate writable "upper" directory.  The container sees one coherent
// filesystem, but the image layers are never modified.
//
//  Directory layout:
//
//   <work-dir>/
//   ├── lower/          ← read-only image rootfs (bind-mount of the image)
//   ├── upper/          ← all writes go here (initially empty)
//   ├── work/           ← kernel internal scratch dir (must be on same FS as upper)
//   └── merged/         ← the container's / (overlayfs mount point)
//
//  The kernel OverlayFS mount command:
//
//   mount -t overlay overlay \
//     -o lowerdir=<lower>,upperdir=<upper>,workdir=<work> \
//     <merged>
//
// How changes propagate:
//   READ   → served from upper if the file exists there; otherwise from lower.
//   WRITE  → a copy of the lower file is brought into upper, then modified.
//   DELETE → a "whiteout" character device (0,0) is created in upper.
//
// The image is thus never modified — you can start a hundred containers from
// the same image simultaneously, each with its own writable upper layer.
//
// Multi-layer stacking:
//   lowerdir accepts a colon-separated list:  layer3:layer2:layer1
//   Layers are evaluated right-to-left (rightmost = oldest/bottom layer).
//   This mirrors OCI image layer ordering (first entry in manifest = bottom).
//
// Kernel requirements:
//   CONFIG_OVERLAY_FS (default in Ubuntu, Fedora, Debian kernels)
//   Usually available in WSL 2 without any changes.

package rootfs

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"syscall"
)

// OverlayDirs groups the four directories that describe one overlay mount.
type OverlayDirs struct {
	Lower  []string // ordered image layers (first = bottom/oldest)
	Upper  string   // writable container layer
	Work   string   // kernel scratch dir (same filesystem as Upper)
	Merged string   // the final merged view — container's /
}

// validateOverlayOptionPath rejects path bytes that have syntax in the legacy
// mount(2) option string. OverlayFS parses options as a comma-separated list;
// lowerdir additionally uses ':' to separate layers. Backslash is rejected as
// well so callers cannot escape or reinterpret those delimiters. Failing closed
// is safer than trying to hand-roll escaping for kernel option parsing.
func validateOverlayOptionPath(kind, path string, lowerdir bool) error {
	if path == "" {
		return fmt.Errorf("overlay %s path is empty", kind)
	}
	if strings.ContainsAny(path, ",\\") {
		return fmt.Errorf("overlay %s path %q contains unsupported mount-option delimiter", kind, path)
	}
	if lowerdir && strings.Contains(path, ":") {
		return fmt.Errorf("overlay %s path %q contains unsupported lowerdir delimiter ':'", kind, path)
	}
	return nil
}

func validateOverlayDirs(dirs *OverlayDirs) error {
	if dirs == nil {
		return fmt.Errorf("overlay directories are nil")
	}
	for i, lower := range dirs.Lower {
		if err := validateOverlayOptionPath(fmt.Sprintf("lower[%d]", i), lower, true); err != nil {
			return err
		}
	}
	if err := validateOverlayOptionPath("upper", dirs.Upper, false); err != nil {
		return err
	}
	if err := validateOverlayOptionPath("work", dirs.Work, false); err != nil {
		return err
	}
	return nil
}

// PrepareOverlay creates the overlay work directories under containerDir,
// mounts the overlayfs, and returns the merged path (the container's rootfs).
//
//   imageRootFS  — path to the extracted (read-only) image rootfs
//   containerDir — a per-container directory to hold upper, work, merged
//
// After the call, containerDir/merged is ready to use as a rootfs.
// The mount lives only in the current mount namespace; when the container
// exits and its namespace is destroyed, the overlay is automatically
// unmounted by the kernel.
func PrepareOverlay(imageRootFS, containerDir string) (*OverlayDirs, error) {
	dirs := &OverlayDirs{
		Lower:  []string{imageRootFS},
		Upper:  filepath.Join(containerDir, "upper"),
		Work:   filepath.Join(containerDir, "work"),
		Merged: filepath.Join(containerDir, "merged"),
	}
	if err := validateOverlayDirs(dirs); err != nil {
		return nil, err
	}

	for _, d := range []string{dirs.Upper, dirs.Work, dirs.Merged} {
		if err := os.MkdirAll(d, 0755); err != nil {
			return nil, fmt.Errorf("overlay mkdir %s: %w", d, err)
		}
	}

	// Build the mount(2) options string:
	//   lowerdir=<l1>:<l2>,upperdir=<u>,workdir=<w>
	//
	// The kernel reads this from the mount(2) data parameter.
	// Paths in lowerdir must be colon-separated and may not contain colons
	// themselves (a limitation of the overlayfs options parser).
	lowerStr := strings.Join(dirs.Lower, ":")
	options := fmt.Sprintf("lowerdir=%s,upperdir=%s,workdir=%s",
		lowerStr, dirs.Upper, dirs.Work)

	if err := syscall.Mount("overlay", dirs.Merged, "overlay", 0, options); err != nil {
		return nil, fmt.Errorf("mount overlay: %w\n"+
			"  (If you see EPERM, the kernel may require root or user_xattr. "+
			"  Try 'sudo' or check /proc/filesystems for 'overlay')", err)
	}

	return dirs, nil
}

// IsMounted returns true if overlayfs is available on this kernel.
// Checks /proc/filesystems, which lists all built-in and loaded modules.
func IsMounted() bool {
	data, err := os.ReadFile("/proc/filesystems")
	if err != nil {
		return false
	}
	for _, line := range strings.Split(string(data), "\n") {
		if strings.Contains(line, "overlay") {
			return true
		}
	}
	return false
}

// PrepareOverlayMultiLayer creates an overlay mount from an ordered list of
// extracted layer directories (oldest first = index 0).
//
// In a real OCI runtime, each layer is a separate directory extracted from
// the image manifest.  This function is the multi-layer version of PrepareOverlay.
//
// The lowerdir string to the kernel is built as:
//   topmost_layer:...:bottom_layer
// i.e. the layers slice reversed and joined with colons.
func PrepareOverlayMultiLayer(layers []string, containerDir string) (*OverlayDirs, error) {
	if len(layers) == 0 {
		return nil, fmt.Errorf("overlay: at least one layer required")
	}

	// Reverse: kernel wants topmost layer first in lowerdir.
	reversed := make([]string, len(layers))
	for i, l := range layers {
		reversed[len(layers)-1-i] = l
	}

	dirs := &OverlayDirs{
		Lower:  reversed,
		Upper:  filepath.Join(containerDir, "upper"),
		Work:   filepath.Join(containerDir, "work"),
		Merged: filepath.Join(containerDir, "merged"),
	}
	if err := validateOverlayDirs(dirs); err != nil {
		return nil, err
	}

	for _, d := range []string{dirs.Upper, dirs.Work, dirs.Merged} {
		if err := os.MkdirAll(d, 0755); err != nil {
			return nil, fmt.Errorf("overlay mkdir %s: %w", d, err)
		}
	}

	lowerStr := strings.Join(dirs.Lower, ":")
	options := fmt.Sprintf("lowerdir=%s,upperdir=%s,workdir=%s",
		lowerStr, dirs.Upper, dirs.Work)

	if err := syscall.Mount("overlay", dirs.Merged, "overlay", 0, options); err != nil {
		return nil, fmt.Errorf("mount overlay (%d layers): %w", len(layers), err)
	}

	return dirs, nil
}
