package logs

import (
	"os"
	"path/filepath"
	"testing"
)

func TestFileInfoLinkCountUsesCapturedSnapshot(t *testing.T) {
	tmpDir := t.TempDir()
	archivePath := filepath.Join(tmpDir, "container.log.1.gz")
	linkPath := filepath.Join(tmpDir, "archive-link.gz")

	if err := os.WriteFile(archivePath, []byte("archive"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.Link(archivePath, linkPath); err != nil {
		t.Fatal(err)
	}

	info, err := os.Lstat(archivePath)
	if err != nil {
		t.Fatal(err)
	}
	count, err := fileInfoLinkCount(info)
	if err != nil {
		t.Fatal(err)
	}
	if count != 2 {
		t.Fatalf("link count=%d, want 2", count)
	}

	if err := os.Remove(linkPath); err != nil {
		t.Fatal(err)
	}
	count, err = fileInfoLinkCount(info)
	if err != nil {
		t.Fatal(err)
	}
	if count != 2 {
		t.Fatalf("captured snapshot link count=%d after pathname mutation, want 2", count)
	}
}
