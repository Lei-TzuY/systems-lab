//go:build !linux

package state

func imageMetadataComponentLimit(string) (int, bool) {
	return maxLegacyImageMetadataFilenameBytes, true
}
