//go:build linux

package image

import (
	"fmt"
	"sort"

	"golang.org/x/sys/unix"
)

func finalizeDirectoryMetadata(destDir string, dirs []directoryMetadata) error {
	if len(dirs) == 0 { return nil }
	root, err := openExtractionRoot(destDir)
	if err != nil { return err }
	defer root.Close()

	ordered := append([]directoryMetadata(nil), dirs...)
	sort.SliceStable(ordered, func(i, j int) bool { return len(ordered[i].target) > len(ordered[j].target) })
	for _, meta := range ordered {
		parent, err := root.openParent(meta.target, "directory metadata", false)
		if err != nil { return err }
		fd, err := unix.Openat(parent.fd, parent.leaf, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
		parent.Close()
		if err != nil { return fmt.Errorf("open directory for metadata %s: %w", meta.target, err) }
		if err := restoreOwnershipFD(fd, meta.target, meta.uid, meta.gid); err != nil { _ = unix.Close(fd); return err }
		if err := unix.Fchmod(fd, tarUnixMode(meta.mode)); err != nil { _ = unix.Close(fd); return fmt.Errorf("chmod directory %s: %w", meta.target, err) }
		if err := restoreXattrsFD(fd, meta.target, meta.xattrs); err != nil { _ = unix.Close(fd); return err }
		if !meta.modTime.IsZero() {
			tv := unix.NsecToTimeval(meta.modTime.UnixNano())
			if err := unix.Futimes(fd, []unix.Timeval{tv, tv}); err != nil { _ = unix.Close(fd); return fmt.Errorf("restore directory mtime %s: %w", meta.target, err) }
		}
		if err := unix.Close(fd); err != nil { return fmt.Errorf("close directory metadata fd %s: %w", meta.target, err) }
	}
	return nil
}
