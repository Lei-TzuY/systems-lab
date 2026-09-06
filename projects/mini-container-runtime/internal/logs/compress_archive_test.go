package logs

import (
	"compress/gzip"
	"errors"
	"os"
	"path/filepath"
	"testing"
)

func TestCompressRotatedLog(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log.1")
	_ = os.WriteFile(logPath, []byte("log data content"), 0644)

	if err := CompressRotatedLog(logPath); err != nil {
		t.Fatalf("CompressRotatedLog error: %v", err)
	}

	if _, err := os.Stat(logPath + ".gz"); err != nil {
		t.Fatalf("Compressed archive container.log.1.gz does not exist: %v", err)
	}
}

func TestCompressRotatedLogReportsSourceRemovalFailure(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log.1")
	if err := os.WriteFile(logPath, []byte("log data content"), 0644); err != nil {
		t.Fatal(err)
	}

	oldRemove := compressArchiveRemove
	wantErr := errors.New("remove failed")
	compressArchiveRemove = func(string) error { return wantErr }
	defer func() { compressArchiveRemove = oldRemove }()

	err := CompressRotatedLog(logPath)
	if err == nil {
		t.Fatal("expected source removal failure to be reported")
	}
	if !errors.Is(err, wantErr) {
		t.Fatalf("CompressRotatedLog error = %v, want wrapped removal failure", err)
	}
	if _, statErr := os.Stat(logPath); statErr != nil {
		t.Fatalf("source log should remain after failed removal: %v", statErr)
	}
}

func TestCompressRotatedLogReportsGzipFinalizeFailure(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log.1")
	if err := os.WriteFile(logPath, []byte("log data content"), 0644); err != nil {
		t.Fatal(err)
	}

	oldClose := compressArchiveGzipClose
	wantErr := errors.New("gzip finalize failed")
	compressArchiveGzipClose = func(*gzip.Writer) error { return wantErr }
	defer func() { compressArchiveGzipClose = oldClose }()

	err := CompressRotatedLog(logPath)
	if err == nil {
		t.Fatal("expected gzip finalize failure to be reported")
	}
	if !errors.Is(err, wantErr) {
		t.Fatalf("CompressRotatedLog error = %v, want wrapped finalize failure", err)
	}
	if _, statErr := os.Stat(logPath); statErr != nil {
		t.Fatalf("source log should remain after failed gzip finalize: %v", statErr)
	}
}

func TestCompressRotatedLogRejectsSymlinkDestination(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log.1")
	if err := os.WriteFile(logPath, []byte("log data content"), 0644); err != nil {
		t.Fatal(err)
	}

	victimPath := filepath.Join(tmpDir, "victim")
	wantVictim := []byte("do not overwrite")
	if err := os.WriteFile(victimPath, wantVictim, 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(victimPath, logPath+".gz"); err != nil {
		t.Fatal(err)
	}

	if err := CompressRotatedLog(logPath); err == nil {
		t.Fatal("expected symlink destination to be rejected")
	}

	gotVictim, err := os.ReadFile(victimPath)
	if err != nil {
		t.Fatal(err)
	}
	if string(gotVictim) != string(wantVictim) {
		t.Fatalf("symlink target was modified: got %q, want %q", gotVictim, wantVictim)
	}
	if _, err := os.Stat(logPath); err != nil {
		t.Fatalf("source log should remain after unsafe destination rejection: %v", err)
	}
}

func TestCompressRotatedLogRequiresDurableArchiveBeforeSourceRemoval(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log.1")
	if err := os.WriteFile(logPath, []byte("log data content"), 0644); err != nil {
		t.Fatal(err)
	}

	oldSync := compressArchiveSync
	wantErr := errors.New("sync failed")
	compressArchiveSync = func(*os.File) error { return wantErr }
	defer func() { compressArchiveSync = oldSync }()

	err := CompressRotatedLog(logPath)
	if err == nil {
		t.Fatal("expected archive sync failure to be reported before source removal")
	}
	if !errors.Is(err, wantErr) {
		t.Fatalf("CompressRotatedLog error = %v, want wrapped sync failure", err)
	}
	if _, statErr := os.Stat(logPath); statErr != nil {
		t.Fatalf("source log should remain when archive durability is unconfirmed: %v", statErr)
	}
}

func TestCompressRotatedLogRequiresDurableArchiveDirectoryBeforeSourceRemoval(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log.1")
	if err := os.WriteFile(logPath, []byte("log data content"), 0644); err != nil {
		t.Fatal(err)
	}

	oldSyncDir := compressArchiveSyncDir
	wantErr := errors.New("directory sync failed")
	compressArchiveSyncDir = func(string) error { return wantErr }
	defer func() { compressArchiveSyncDir = oldSyncDir }()

	err := CompressRotatedLog(logPath)
	if err == nil {
		t.Fatal("expected archive directory sync failure to be reported before source removal")
	}
	if !errors.Is(err, wantErr) {
		t.Fatalf("CompressRotatedLog error = %v, want wrapped directory sync failure", err)
	}
	if _, statErr := os.Stat(logPath); statErr != nil {
		t.Fatalf("source log should remain when archive directory durability is unconfirmed: %v", statErr)
	}
}

func TestCompressRotatedLogReportsSourceRemovalDirectorySyncFailure(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log.1")
	if err := os.WriteFile(logPath, []byte("log data content"), 0644); err != nil {
		t.Fatal(err)
	}

	oldSyncDir := compressArchiveSyncDir
	wantErr := errors.New("directory sync failed")
	calls := 0
	compressArchiveSyncDir = func(string) error {
		calls++
		if calls == 2 {
			return wantErr
		}
		return nil
	}
	defer func() { compressArchiveSyncDir = oldSyncDir }()

	err := CompressRotatedLog(logPath)
	if err == nil {
		t.Fatal("expected source removal directory sync failure to be reported")
	}
	if !errors.Is(err, wantErr) {
		t.Fatalf("CompressRotatedLog error = %v, want wrapped directory sync failure", err)
	}
	if calls != 2 {
		t.Fatalf("directory sync calls = %d, want 2", calls)
	}
	if _, statErr := os.Stat(logPath); !os.IsNotExist(statErr) {
		t.Fatalf("source log should already be removed when post-remove directory sync fails, stat err = %v", statErr)
	}
}

func TestCompressRotatedLogReportsArchiveCloseFailure(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log.1")
	if err := os.WriteFile(logPath, []byte("log data content"), 0644); err != nil {
		t.Fatal(err)
	}

	oldClose := compressArchiveFileClose
	wantErr := errors.New("close failed")
	compressArchiveFileClose = func(*os.File) error { return wantErr }
	defer func() { compressArchiveFileClose = oldClose }()

	err := CompressRotatedLog(logPath)
	if err == nil {
		t.Fatal("expected archive close failure to be reported before source removal")
	}
	if !errors.Is(err, wantErr) {
		t.Fatalf("CompressRotatedLog error = %v, want wrapped close failure", err)
	}
	if _, statErr := os.Stat(logPath); statErr != nil {
		t.Fatalf("source log should remain after archive close failure: %v", statErr)
	}
}

func TestCompressRotatedLogRejectsReplacedSourceBeforeRemoval(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log.1")
	originalPath := filepath.Join(tmpDir, "original-preserved")
	if err := os.WriteFile(logPath, []byte("original log data"), 0644); err != nil {
		t.Fatal(err)
	}

	oldClose := compressArchiveFileClose
	compressArchiveFileClose = func(f *os.File) error {
		if err := f.Close(); err != nil {
			return err
		}
		if err := os.Rename(logPath, originalPath); err != nil {
			return err
		}
		return os.WriteFile(logPath, []byte("replacement log data"), 0644)
	}
	defer func() { compressArchiveFileClose = oldClose }()

	err := CompressRotatedLog(logPath)
	if err == nil {
		t.Fatal("expected replaced source identity to be rejected before removal")
	}

	got, statErr := os.ReadFile(logPath)
	if statErr != nil {
		t.Fatalf("replacement source should remain untouched: %v", statErr)
	}
	if string(got) != "replacement log data" {
		t.Fatalf("replacement source content = %q, want %q", got, "replacement log data")
	}
	if _, statErr := os.Stat(originalPath); statErr != nil {
		t.Fatalf("original source should remain at preserved path: %v", statErr)
	}
}

func TestCompressRotatedLogRejectsSymlinkSource(t *testing.T) {
	tmpDir := t.TempDir()
	victimPath := filepath.Join(tmpDir, "victim.log")
	wantVictim := []byte("victim log data")
	if err := os.WriteFile(victimPath, wantVictim, 0644); err != nil {
		t.Fatal(err)
	}

	logPath := filepath.Join(tmpDir, "container.log.1")
	if err := os.Symlink(victimPath, logPath); err != nil {
		t.Fatal(err)
	}

	if err := CompressRotatedLog(logPath); err == nil {
		t.Fatal("expected symlink source to be rejected")
	}

	gotVictim, err := os.ReadFile(victimPath)
	if err != nil {
		t.Fatal(err)
	}
	if string(gotVictim) != string(wantVictim) {
		t.Fatalf("symlink target was modified: got %q, want %q", gotVictim, wantVictim)
	}
	if _, err := os.Lstat(logPath); err != nil {
		t.Fatalf("symlink source should remain untouched: %v", err)
	}
	if _, err := os.Stat(logPath + ".gz"); !os.IsNotExist(err) {
		t.Fatalf("gzip archive should not be created for symlink source, stat err = %v", err)
	}
}
