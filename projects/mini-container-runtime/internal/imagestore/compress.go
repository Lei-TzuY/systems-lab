package imagestore

import (
	"bytes"
)

// DetectCompressionFormat inspects magic bytes to detect tarball compression format.
func DetectCompressionFormat(header []byte) string {
	if len(header) >= 2 && header[0] == 0x1f && header[1] == 0x8b {
		return "gzip"
	}
	if len(header) >= 4 && bytes.Equal(header[:4], []byte{0x28, 0xb5, 0x2f, 0xfd}) {
		return "zstd"
	}
	if len(header) >= 6 && bytes.Equal(header[:6], []byte{0xfd, '7', 'z', 'X', 'Z', 0x00}) {
		return "xz"
	}
	return "raw-tar"
}
