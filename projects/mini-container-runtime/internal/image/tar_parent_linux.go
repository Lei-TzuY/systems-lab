//go:build linux

package image

// Linux entry-specific extractors create and traverse parents relative to a
// pinned extraction-root dirfd. A pathname-based preflight would only add a
// second, racy filesystem mutation before the hardened operation.
func prepareTarEntryParent(target, destDir string) error {
	return nil
}
