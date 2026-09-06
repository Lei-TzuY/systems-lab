package image

// PublishDirectoryNoReplace atomically publishes a staged directory without
// replacing an existing destination. Linux uses renameat2(RENAME_NOREPLACE);
// portable fallbacks preserve the package's existing best-effort semantics.
func PublishDirectoryNoReplace(staging, destination string) error {
	return publishDirectoryNoReplace(staging, destination)
}
