//go:build linux

package image

import (
	"os"

	"golang.org/x/sys/unix"
)

// tarUnixMode converts Go's FileMode representation into the permission and
// special-mode bits expected by Linux chmod/mknod. FileMode.Perm deliberately
// excludes setuid, setgid, and sticky, so using it alone silently changes tar
// filesystem semantics (for example /tmp without its sticky bit).
func tarUnixMode(mode os.FileMode) uint32 {
	bits := uint32(mode.Perm())
	if mode&os.ModeSetuid != 0 {
		bits |= unix.S_ISUID
	}
	if mode&os.ModeSetgid != 0 {
		bits |= unix.S_ISGID
	}
	if mode&os.ModeSticky != 0 {
		bits |= unix.S_ISVTX
	}
	return bits
}
