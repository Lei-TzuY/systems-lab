package logs

import (
	"errors"
	"os"
	"path/filepath"
	"testing"
)

func TestArchiveLogFile(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log")
	_ = os.WriteFile(logPath, []byte("content"), 0644)

	if err := ArchiveLogFile(logPath, 3); err != nil {
		t.Fatalf("ArchiveLogFile error: %v", err)
	}

	if _, err := os.Stat(logPath + ".1"); err != nil {
		t.Fatalf("Archived log file container.log.1 does not exist: %v", err)
	}
}

func TestArchiveLogFileReportsActiveRenameFailure(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log")
	if err := os.WriteFile(logPath, []byte("content"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(logPath+".1", 0755); err != nil {
		t.Fatal(err)
	}

	if err := ArchiveLogFile(logPath, 3); err == nil {
		t.Fatal("expected archive failure when destination is a directory")
	}

	got, err := os.ReadFile(logPath)
	if err != nil {
		t.Fatalf("active log should remain after failed rename: %v", err)
	}
	if string(got) != "content" {
		t.Fatalf("active log changed after failed rename: %q", got)
	}
}

func TestArchiveLogFileRejectsDanglingActiveSymlink(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log")
	target := filepath.Join(tmpDir, "missing-target")
	if err := os.Symlink(target, logPath); err != nil {
		t.Fatal(err)
	}

	if err := ArchiveLogFile(logPath, 3); err == nil {
		t.Fatal("expected archive failure for dangling active symlink")
	}

	fi, err := os.Lstat(logPath)
	if err != nil {
		t.Fatalf("dangling symlink should remain after rejection: %v", err)
	}
	if fi.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("active path mode = %v, want symlink", fi.Mode())
	}
}

func TestArchiveLogFileRejectsActiveIdentityReplacement(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log")
	originalPath := filepath.Join(tmpDir, "original.log")
	if err := os.WriteFile(logPath, []byte("original"), 0644); err != nil {
		t.Fatal(err)
	}

	oldLstat := archiveLstat
	replaced := false
	archiveLstat = func(path string) (os.FileInfo, error) {
		fi, err := os.Lstat(path)
		if err == nil && path == logPath && !replaced {
			replaced = true
			if err := os.Rename(logPath, originalPath); err != nil {
				t.Fatalf("move inspected log: %v", err)
			}
			if err := os.WriteFile(logPath, []byte("replacement"), 0644); err != nil {
				t.Fatalf("replace inspected log: %v", err)
			}
		}
		return fi, err
	}
	defer func() { archiveLstat = oldLstat }()

	if err := ArchiveLogFile(logPath, 3); err == nil {
		t.Fatal("expected archive failure after active-log identity replacement")
	}

	got, err := os.ReadFile(logPath)
	if err != nil {
		t.Fatalf("replacement path should remain untouched: %v", err)
	}
	if string(got) != "replacement" {
		t.Fatalf("replacement path changed: %q", got)
	}
	got, err = os.ReadFile(originalPath)
	if err != nil {
		t.Fatalf("original inspected inode should remain available: %v", err)
	}
	if string(got) != "original" {
		t.Fatalf("original inspected inode changed: %q", got)
	}
}

func TestArchiveLogFileSyncsDirectoryAfterActiveRename(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log")
	if err := os.WriteFile(logPath, []byte("content"), 0644); err != nil {
		t.Fatal(err)
	}

	oldSyncDir := archiveSyncDir
	calls := 0
	archiveSyncDir = func(dir string) error {
		calls++
		if dir != tmpDir {
			t.Fatalf("sync dir = %q, want %q", dir, tmpDir)
		}
		return nil
	}
	defer func() { archiveSyncDir = oldSyncDir }()

	if err := ArchiveLogFile(logPath, 3); err != nil {
		t.Fatalf("ArchiveLogFile error: %v", err)
	}
	if calls != 1 {
		t.Fatalf("directory sync calls = %d, want 1 after active rename", calls)
	}
}

func TestArchiveLogFileReportsDirectorySyncFailure(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log")
	if err := os.WriteFile(logPath, []byte("content"), 0644); err != nil {
		t.Fatal(err)
	}

	oldSyncDir := archiveSyncDir
	wantErr := errors.New("sync failed")
	archiveSyncDir = func(string) error { return wantErr }
	defer func() { archiveSyncDir = oldSyncDir }()

	err := ArchiveLogFile(logPath, 3)
	if err == nil {
		t.Fatal("expected directory sync failure to be reported")
	}
	if !errors.Is(err, wantErr) {
		t.Fatalf("ArchiveLogFile error = %v, want wrapped sync failure", err)
	}
	if _, statErr := os.Stat(logPath + ".1"); statErr != nil {
		t.Fatalf("rename should have completed before durability failure: %v", statErr)
	}
}

func TestArchiveLogFileDoesNotClobberDestinationCreatedAfterSourceRevalidation(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log")
	dst := logPath + ".1"
	if err := os.WriteFile(logPath, []byte("source"), 0644); err != nil {
		t.Fatal(err)
	}

	oldLstat := archiveLstat
	calls := 0
	archiveLstat = func(path string) (os.FileInfo, error) {
		fi, err := os.Lstat(path)
		if path == logPath && err == nil {
			calls++
			if calls == 2 {
				if err := os.WriteFile(dst, []byte("concurrent-destination"), 0644); err != nil {
					t.Fatalf("create concurrent destination: %v", err)
				}
			}
		}
		return fi, err
	}
	defer func() { archiveLstat = oldLstat }()

	if err := ArchiveLogFile(logPath, 3); err == nil {
		t.Fatal("expected archive failure when destination appears before rename")
	}

	got, err := os.ReadFile(dst)
	if err != nil {
		t.Fatalf("concurrent destination should remain: %v", err)
	}
	if string(got) != "concurrent-destination" {
		t.Fatalf("destination was clobbered: %q", got)
	}
	got, err = os.ReadFile(logPath)
	if err != nil {
		t.Fatalf("source should remain after no-clobber failure: %v", err)
	}
	if string(got) != "source" {
		t.Fatalf("source changed after no-clobber failure: %q", got)
	}
}
