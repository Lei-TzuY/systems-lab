package logs

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestPruneRotatedLogs(t *testing.T) {
	tmpDir := t.TempDir()
	logFile := filepath.Join(tmpDir, "container.log.1.gz")
	_ = os.WriteFile(logFile, []byte("old log"), 0644)

	count, err := PruneRotatedLogs(tmpDir, 1*time.Millisecond)
	if err != nil && count == 0 {
		t.Fatalf("PruneRotatedLogs error: %v", err)
	}
}

func TestPruneRotatedLogsLeavesUnrelatedGzipFile(t *testing.T) {
	tmpDir := t.TempDir()
	backup := filepath.Join(tmpDir, "backup.gz")
	if err := os.WriteFile(backup, []byte("unrelated backup"), 0644); err != nil {
		t.Fatalf("write unrelated gzip: %v", err)
	}
	old := time.Now().Add(-2 * time.Hour)
	if err := os.Chtimes(backup, old, old); err != nil {
		t.Fatalf("age unrelated gzip: %v", err)
	}

	count, err := PruneRotatedLogs(tmpDir, time.Hour)
	if err != nil {
		t.Fatalf("PruneRotatedLogs error: %v", err)
	}
	if count != 0 {
		t.Fatalf("deleted %d files, want 0", count)
	}
	if _, err := os.Stat(backup); err != nil {
		t.Fatalf("unrelated gzip was removed: %v", err)
	}
}

func TestPruneRotatedLogsLeavesNonRotationLogSuffix(t *testing.T) {
	tmpDir := t.TempDir()
	backup := filepath.Join(tmpDir, "audit.log.backup")
	if err := os.WriteFile(backup, []byte("not a rotated log"), 0644); err != nil {
		t.Fatalf("write unrelated log-suffixed file: %v", err)
	}
	old := time.Now().Add(-2 * time.Hour)
	if err := os.Chtimes(backup, old, old); err != nil {
		t.Fatalf("age unrelated log-suffixed file: %v", err)
	}

	count, err := PruneRotatedLogs(tmpDir, time.Hour)
	if err != nil {
		t.Fatalf("PruneRotatedLogs error: %v", err)
	}
	if count != 0 {
		t.Fatalf("deleted %d files, want 0", count)
	}
	if _, err := os.Stat(backup); err != nil {
		t.Fatalf("unrelated log-suffixed file was removed: %v", err)
	}
}
