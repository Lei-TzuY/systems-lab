package image

import (
	"os"
	"path/filepath"
	"testing"
)

func TestExportDirAndUnpackRoundTrip(t *testing.T) {
	srcDir := filepath.Join(t.TempDir(), "src")
	destDir := filepath.Join(t.TempDir(), "dest")
	archivePath := filepath.Join(t.TempDir(), "export.tar.gz")

	// Setup source structure
	if err := os.MkdirAll(filepath.Join(srcDir, "bin"), 0755); err != nil {
		t.Fatalf("MkdirAll: %v", err)
	}
	if err := os.WriteFile(filepath.Join(srcDir, "bin", "app"), []byte("echo hello\n"), 0755); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}
	if err := os.WriteFile(filepath.Join(srcDir, "config.txt"), []byte("key=val\n"), 0644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}

	// Export srcDir to tar.gz
	if err := ExportDir(srcDir, archivePath); err != nil {
		t.Fatalf("ExportDir: %v", err)
	}

	// Unpack to destDir
	if err := Unpack(archivePath, destDir); err != nil {
		t.Fatalf("Unpack: %v", err)
	}

	// Verify unpacked files match
	assertFile(t, filepath.Join(destDir, "bin", "app"), "echo hello\n")
	assertFile(t, filepath.Join(destDir, "config.txt"), "key=val\n")
}
