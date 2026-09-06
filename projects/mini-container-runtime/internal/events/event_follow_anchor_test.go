package events

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

func TestInspectEventLogGenerationDetectsFastRegrowthWithOversizedFirstRecord(t *testing.T) {
	path := filepath.Join(t.TempDir(), "events.log")
	oldGeneration := bytes.Repeat([]byte("a"), eventGenerationAnchorLimit+2048)
	if err := os.WriteFile(path, oldGeneration, 0o600); err != nil {
		t.Fatalf("write old generation: %v", err)
	}

	f, err := os.Open(path)
	if err != nil {
		t.Fatalf("open event log: %v", err)
	}
	defer f.Close()
	if _, err := f.Seek(int64(len(oldGeneration)), 0); err != nil {
		t.Fatalf("seek old generation: %v", err)
	}
	anchor, err := readEventGenerationAnchor(f)
	if err != nil {
		t.Fatalf("read old anchor: %v", err)
	}
	if len(anchor) != eventGenerationAnchorLimit {
		t.Fatalf("anchor length = %d, want %d", len(anchor), eventGenerationAnchorLimit)
	}

	// Copytruncate can preserve the inode and regrow beyond the follower's old
	// offset before the next poll. Size and identity alone therefore cannot
	// distinguish this replacement generation.
	newGeneration := bytes.Repeat([]byte("b"), len(oldGeneration)+1024)
	if err := os.WriteFile(path, newGeneration, 0o600); err != nil {
		t.Fatalf("rewrite event log in place: %v", err)
	}

	reopen, _, err := inspectEventLogGeneration(f, path, 0, anchor)
	if err != nil {
		t.Fatalf("inspect generation: %v", err)
	}
	if !reopen {
		t.Fatal("expected oversized first-record prefix change to force reopen")
	}
}

func TestInspectEventLogGenerationAllowsAppendGrowthFromPartialAnchor(t *testing.T) {
	path := filepath.Join(t.TempDir(), "events.log")
	prefix := []byte(`{"timestamp":"2026-09-01T00:00:00Z","type":"start"`)
	if err := os.WriteFile(path, prefix, 0o600); err != nil {
		t.Fatalf("write partial record: %v", err)
	}

	f, err := os.Open(path)
	if err != nil {
		t.Fatalf("open event log: %v", err)
	}
	defer f.Close()
	anchor, err := readEventGenerationAnchor(f)
	if err != nil {
		t.Fatalf("read partial anchor: %v", err)
	}

	if file, err := os.OpenFile(path, os.O_WRONLY|os.O_APPEND, 0); err != nil {
		t.Fatalf("open append: %v", err)
	} else {
		if _, err := file.WriteString(`,"container_id":"abcdef"}` + "\n"); err != nil {
			file.Close()
			t.Fatalf("append record: %v", err)
		}
		if err := file.Close(); err != nil {
			t.Fatalf("close append: %v", err)
		}
	}

	reopen, updated, err := inspectEventLogGeneration(f, path, 0, anchor)
	if err != nil {
		t.Fatalf("inspect append growth: %v", err)
	}
	if reopen {
		t.Fatal("append-only growth must not look like a generation reset")
	}
	if !bytes.Equal(updated, anchor) {
		t.Fatalf("anchor changed after append-only growth: got %q want %q", updated, anchor)
	}
}
