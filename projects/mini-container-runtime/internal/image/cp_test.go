package image

import (
	"os"
	"path/filepath"
	"testing"
)

func TestParseCopyTarget(t *testing.T) {
	tests := []struct {
		in        string
		wantID    string
		wantPath  string
	}{
		{"a3f8b2c1:/tmp/foo.txt", "a3f8b2c1", "/tmp/foo.txt"},
		{"/tmp/hostfile.txt", "", "/tmp/hostfile.txt"},
		{"./local/dir", "", "./local/dir"},
	}

	for _, tt := range tests {
		t.Run(tt.in, func(t *testing.T) {
			id, path := ParseCopyTarget(tt.in)
			if id != tt.wantID || path != tt.wantPath {
				t.Errorf("ParseCopyTarget(%q) = (%q, %q), want (%q, %q)",
					tt.in, id, path, tt.wantID, tt.wantPath)
			}
		})
	}
}

func TestCopyPath(t *testing.T) {
	tmpDir := t.TempDir()
	srcFile := filepath.Join(tmpDir, "src.txt")
	dstFile := filepath.Join(tmpDir, "dst.txt")

	content := "hello minictl cp"
	if err := os.WriteFile(srcFile, []byte(content), 0644); err != nil {
		t.Fatalf("write src: %v", err)
	}

	if err := CopyPath(srcFile, dstFile); err != nil {
		t.Fatalf("CopyPath error: %v", err)
	}

	got, err := os.ReadFile(dstFile)
	if err != nil {
		t.Fatalf("read dst: %v", err)
	}

	if string(got) != content {
		t.Fatalf("got %q, want %q", string(got), content)
	}
}
