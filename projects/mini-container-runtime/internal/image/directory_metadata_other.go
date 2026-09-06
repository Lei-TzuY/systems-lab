//go:build !linux

package image

import "os"

func finalizeDirectoryMetadata(_ string, dirs []directoryMetadata) error {
	for i := len(dirs) - 1; i >= 0; i-- {
		meta := dirs[i]
		if err := os.Chmod(meta.target, meta.mode.Perm()); err != nil {
			return err
		}
		if !meta.modTime.IsZero() {
			if err := os.Chtimes(meta.target, meta.modTime, meta.modTime); err != nil {
				return err
			}
		}
	}
	return nil
}
