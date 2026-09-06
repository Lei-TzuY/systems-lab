//go:build linux

package state

import "golang.org/x/sys/unix"

func imageMetadataComponentLimit(dir string) (int, bool) {
	var fs unix.Statfs_t
	if err := unix.Statfs(dir, &fs); err != nil || fs.Namelen <= 0 {
		return 0, false
	}
	limit := int(fs.Namelen)
	if limit > maxLegacyImageMetadataFilenameBytes {
		limit = maxLegacyImageMetadataFilenameBytes
	}
	return limit, true
}
