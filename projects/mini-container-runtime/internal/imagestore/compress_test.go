package imagestore

import (
	"testing"
)

func TestDetectCompressionFormat(t *testing.T) {
	gzipHeader := []byte{0x1f, 0x8b, 0x08, 0x00}
	zstdHeader := []byte{0x28, 0xb5, 0x2f, 0xfd}
	tarHeader := []byte("some raw tar content")

	if fmt := DetectCompressionFormat(gzipHeader); fmt != "gzip" {
		t.Fatalf("Gzip format = %s, want gzip", fmt)
	}
	if fmt := DetectCompressionFormat(zstdHeader); fmt != "zstd" {
		t.Fatalf("Zstd format = %s, want zstd", fmt)
	}
	if fmt := DetectCompressionFormat(tarHeader); fmt != "raw-tar" {
		t.Fatalf("Tar format = %s, want raw-tar", fmt)
	}
}
