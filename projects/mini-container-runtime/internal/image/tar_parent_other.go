//go:build !linux

package image

func prepareTarEntryParent(target, destDir string) error {
	return ensureSafeParentDirs(target, destDir)
}
