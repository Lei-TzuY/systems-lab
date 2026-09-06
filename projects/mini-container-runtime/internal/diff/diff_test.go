package diff

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestDiffUpper(t *testing.T) {
	tmpDir := t.TempDir()
	upper := filepath.Join(tmpDir, "upper")
	_ = os.MkdirAll(filepath.Join(upper, "app"), 0755)
	_ = os.MkdirAll(filepath.Join(upper, "etc"), 0755)

	_ = os.WriteFile(filepath.Join(upper, "app", "server.js"), []byte("console.log('hi')"), 0644)
	_ = os.WriteFile(filepath.Join(upper, "etc", ".wh.oldconfig"), []byte(""), 0644)

	changes, err := DiffUpper(upper)
	if err != nil {
		t.Fatalf("DiffUpper error: %v", err)
	}

	formatted := FormatDiff(changes)
	if !strings.Contains(formatted, "A /app/server.js") {
		t.Fatalf("Missing added file in diff:\n%s", formatted)
	}
	if !strings.Contains(formatted, "D /etc/oldconfig") {
		t.Fatalf("Missing deleted whiteout in diff:\n%s", formatted)
	}
}

func TestDiffDirectories(t *testing.T) {
	tmpDir := t.TempDir()
	base := filepath.Join(tmpDir, "base")
	target := filepath.Join(tmpDir, "target")

	_ = os.MkdirAll(base, 0755)
	_ = os.MkdirAll(target, 0755)

	_ = os.WriteFile(filepath.Join(base, "file1.txt"), []byte("v1"), 0644)
	_ = os.WriteFile(filepath.Join(base, "file2.txt"), []byte("deleted"), 0644)

	_ = os.WriteFile(filepath.Join(target, "file1.txt"), []byte("v2-modified"), 0644)
	_ = os.WriteFile(filepath.Join(target, "file3.txt"), []byte("added"), 0644)

	changes, err := DiffDirectories(base, target)
	if err != nil {
		t.Fatalf("DiffDirectories error: %v", err)
	}

	formatted := FormatDiff(changes)
	if !strings.Contains(formatted, "C /file1.txt") {
		t.Fatalf("Missing changed file in diff:\n%s", formatted)
	}
	if !strings.Contains(formatted, "A /file3.txt") {
		t.Fatalf("Missing added file in diff:\n%s", formatted)
	}
	if !strings.Contains(formatted, "D /file2.txt") {
		t.Fatalf("Missing deleted file in diff:\n%s", formatted)
	}
}

func TestDiffDirectories_SameSizeModification(t *testing.T) {
	tmpDir := t.TempDir()
	base := filepath.Join(tmpDir, "base")
	target := filepath.Join(tmpDir, "target")

	_ = os.MkdirAll(base, 0755)
	_ = os.MkdirAll(target, 0755)

	// Both files have exact same size (10 bytes) but different content
	_ = os.WriteFile(filepath.Join(base, "config.json"), []byte("1234567890"), 0644)
	_ = os.WriteFile(filepath.Join(target, "config.json"), []byte("abcdefghij"), 0644)

	changes, err := DiffDirectories(base, target)
	if err != nil {
		t.Fatalf("DiffDirectories error: %v", err)
	}

	if len(changes) != 1 {
		t.Fatalf("expected 1 change, got %d: %v", len(changes), changes)
	}
	if changes[0].Type != Changed || changes[0].Path != "/config.json" {
		t.Fatalf("expected C /config.json, got %+v", changes[0])
	}
}

func TestDiffDirectories_DeterministicSorting(t *testing.T) {
	tmpDir := t.TempDir()
	base := filepath.Join(tmpDir, "base")
	target := filepath.Join(tmpDir, "target")

	_ = os.MkdirAll(base, 0755)
	_ = os.MkdirAll(target, 0755)

	// Create multiple files in both dirs
	_ = os.WriteFile(filepath.Join(base, "z.txt"), []byte("same"), 0644)
	_ = os.WriteFile(filepath.Join(target, "z.txt"), []byte("same"), 0644)

	_ = os.WriteFile(filepath.Join(base, "b.txt"), []byte("v1"), 0644)
	_ = os.WriteFile(filepath.Join(target, "b.txt"), []byte("v2"), 0644)

	_ = os.WriteFile(filepath.Join(target, "a.txt"), []byte("new"), 0644)
	_ = os.WriteFile(filepath.Join(base, "m.txt"), []byte("old"), 0644)

	changes1, err := DiffDirectories(base, target)
	if err != nil {
		t.Fatalf("DiffDirectories run 1 error: %v", err)
	}

	// Paths should be strictly sorted: /a.txt (A), /b.txt (C), /m.txt (D)
	wantOrder := []struct {
		typ  ChangeType
		path string
	}{
		{Added, "/a.txt"},
		{Changed, "/b.txt"},
		{Deleted, "/m.txt"},
	}

	if len(changes1) != len(wantOrder) {
		t.Fatalf("expected %d changes, got %d", len(wantOrder), len(changes1))
	}
	for i, want := range wantOrder {
		if changes1[i].Type != want.typ || changes1[i].Path != want.path {
			t.Errorf("change[%d] = %+v, want Type=%s Path=%s", i, changes1[i], want.typ, want.path)
		}
	}

	// Repeated runs must produce identical ordering
	for r := 0; r < 5; r++ {
		changesN, _ := DiffDirectories(base, target)
		for i := range changes1 {
			if changesN[i] != changes1[i] {
				t.Fatalf("run %d produced non-deterministic ordering: %+v vs %+v", r, changesN, changes1)
			}
		}
	}
}

func TestDiffUpper_ErrorIntegrity(t *testing.T) {
	// Calling DiffUpper on non-existent directory must return error
	if _, err := DiffUpper(filepath.Join(t.TempDir(), "nonexistent")); err == nil {
		t.Errorf("DiffUpper on nonexistent directory expected error, got nil")
	}
}

func TestDiffDirectories_ErrorIntegrity(t *testing.T) {
	tmpDir := t.TempDir()
	validDir := filepath.Join(tmpDir, "valid")
	_ = os.MkdirAll(validDir, 0755)

	nonexistent := filepath.Join(tmpDir, "nonexistent")

	// Calling DiffDirectories on non-existent base directory must return error
	if _, err := DiffDirectories(nonexistent, validDir); err == nil {
		t.Errorf("DiffDirectories on nonexistent base expected error, got nil")
	}

	// Calling DiffDirectories on non-existent target directory must return error
	if _, err := DiffDirectories(validDir, nonexistent); err == nil {
		t.Errorf("DiffDirectories on nonexistent target expected error, got nil")
	}
}
