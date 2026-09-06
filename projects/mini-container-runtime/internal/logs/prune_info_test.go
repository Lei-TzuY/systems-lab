package logs

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestPruneRotatedLogsFailsClosedWhenCandidateInfoFails(t *testing.T) {
	tmpDir := t.TempDir()
	logFile := filepath.Join(tmpDir, "container.log.1.gz")
	if err := os.WriteFile(logFile, []byte("old log"), 0o644); err != nil {
		t.Fatal(err)
	}
	old := time.Now().Add(-2 * time.Hour)
	if err := os.Chtimes(logFile, old, old); err != nil {
		t.Fatal(err)
	}

	oldHook := pruneBeforeInfo
	pruneBeforeInfo = func(path string) {
		if path == logFile {
			if err := os.Remove(path); err != nil {
				t.Fatalf("remove candidate before metadata lookup: %v", err)
			}
		}
	}
	t.Cleanup(func() { pruneBeforeInfo = oldHook })

	deleted, err := PruneRotatedLogs(tmpDir, time.Hour)
	if err == nil {
		t.Fatalf("expected metadata lookup failure, got nil error (deleted=%d)", deleted)
	}
	if deleted != 0 {
		t.Fatalf("deleted=%d, want 0", deleted)
	}
}
