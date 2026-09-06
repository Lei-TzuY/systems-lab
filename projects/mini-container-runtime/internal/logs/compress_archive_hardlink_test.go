package logs

import (
	"os"
	"path/filepath"
	"testing"
)

func TestCompressRotatedLogRejectsHardLinkedDestinationBeforeTruncate(t *testing.T) {
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
	if err := os.Link(victimPath, logPath+".gz"); err != nil {
		t.Fatal(err)
	}

	if err := CompressRotatedLog(logPath); err == nil {
		t.Fatal("expected hard-linked gzip destination to be rejected")
	}

	gotVictim, err := os.ReadFile(victimPath)
	if err != nil {
		t.Fatal(err)
	}
	if string(gotVictim) != string(wantVictim) {
		t.Fatalf("hard-link target was modified: got %q, want %q", gotVictim, wantVictim)
	}
	if _, err := os.Stat(logPath); err != nil {
		t.Fatalf("source log should remain after unsafe destination rejection: %v", err)
	}
}

func TestCompressRotatedLogRejectsHardLinkedSourceBeforeCreatingArchive(t *testing.T) {
	tmpDir := t.TempDir()
	victimPath := filepath.Join(tmpDir, "victim.log")
	wantVictim := []byte("sensitive victim log\n")
	if err := os.WriteFile(victimPath, wantVictim, 0644); err != nil {
		t.Fatal(err)
	}

	logPath := filepath.Join(tmpDir, "container.log.1")
	if err := os.Link(victimPath, logPath); err != nil {
		t.Fatal(err)
	}

	if err := CompressRotatedLog(logPath); err == nil {
		t.Fatal("expected hard-linked compression source to be rejected")
	}
	if _, err := os.Stat(logPath + ".gz"); !os.IsNotExist(err) {
		t.Fatalf("gzip archive should not be created for unsafe source: %v", err)
	}
	gotVictim, err := os.ReadFile(victimPath)
	if err != nil {
		t.Fatal(err)
	}
	if string(gotVictim) != string(wantVictim) {
		t.Fatalf("hard-link target changed: got %q, want %q", gotVictim, wantVictim)
	}
	if _, err := os.Stat(logPath); err != nil {
		t.Fatalf("source hard link should remain after rejection: %v", err)
	}
}
