package logs

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestStreamLogs(t *testing.T) {
	tmpDir := t.TempDir()
	logFile := filepath.Join(tmpDir, "test.log")
	_ = os.WriteFile(logFile, []byte("line1\nline2\n"), 0644)

	outChan := make(chan string, 10)
	stopChan := make(chan struct{})

	go func() {
		_ = StreamLogs(logFile, outChan, stopChan)
	}()

	time.Sleep(200 * time.Millisecond)
	close(stopChan)

	if len(outChan) < 2 {
		t.Fatalf("StreamLogs received %d lines, want 2", len(outChan))
	}
}
