package events

import (
	"bytes"
	"io"
	"os"
	"path/filepath"
	"testing"
)

func TestInspectEventLogGenerationDetectsRewritePastPreservedPrefix(t *testing.T) {
	path := filepath.Join(t.TempDir(), "events.log")
	prefix := bytes.Repeat([]byte("p"), eventGenerationAnchorLimit)
	oldTail := bytes.Repeat([]byte("a"), eventGenerationAnchorLimit*2)
	oldGeneration := append(bytes.Clone(prefix), oldTail...)
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
		t.Fatalf("read prefix anchor: %v", err)
	}
	checkpoint, err := readEventGenerationCheckpoint(f, 0)
	if err != nil {
		t.Fatalf("read tail checkpoint: %v", err)
	}

	// Preserve the complete prefix anchor while rewriting the already-consumed
	// tail and regrowing beyond the old offset on the same inode.
	newGeneration := append(bytes.Clone(prefix), bytes.Repeat([]byte("b"), len(oldTail)+1024)...)
	if err := os.WriteFile(path, newGeneration, 0o600); err != nil {
		t.Fatalf("rewrite event log in place: %v", err)
	}

	reopen, _, err := inspectEventLogGenerationWithCheckpoint(f, path, 0, anchor, checkpoint)
	if err != nil {
		t.Fatalf("inspect generation: %v", err)
	}
	if !reopen {
		t.Fatal("expected rewrite beyond preserved prefix to force reopen")
	}
}

func TestInspectEventLogGenerationCheckpointAllowsAppendOnlyGrowth(t *testing.T) {
	path := filepath.Join(t.TempDir(), "events.log")
	initial := bytes.Repeat([]byte("x"), eventGenerationAnchorLimit*2)
	if err := os.WriteFile(path, initial, 0o600); err != nil {
		t.Fatalf("write generation: %v", err)
	}

	f, err := os.Open(path)
	if err != nil {
		t.Fatalf("open event log: %v", err)
	}
	defer f.Close()
	if _, err := f.Seek(int64(len(initial)), 0); err != nil {
		t.Fatalf("seek generation: %v", err)
	}
	anchor, err := readEventGenerationAnchor(f)
	if err != nil {
		t.Fatal(err)
	}
	checkpoint, err := readEventGenerationCheckpoint(f, 0)
	if err != nil {
		t.Fatal(err)
	}

	appendFile, err := os.OpenFile(path, os.O_WRONLY|os.O_APPEND, 0)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := appendFile.Write(bytes.Repeat([]byte("y"), 1024)); err != nil {
		appendFile.Close()
		t.Fatal(err)
	}
	if err := appendFile.Close(); err != nil {
		t.Fatal(err)
	}

	reopen, _, err := inspectEventLogGenerationWithCheckpoint(f, path, 0, anchor, checkpoint)
	if err != nil {
		t.Fatalf("inspect append-only growth: %v", err)
	}
	if reopen {
		t.Fatal("append-only growth must preserve checkpoint generation")
	}
}

func TestReadEventGenerationCheckpointDoesNotMoveSequentialOffset(t *testing.T) {
	path := filepath.Join(t.TempDir(), "events.log")
	data := bytes.Repeat([]byte("z"), eventGenerationAnchorLimit*2)
	if err := os.WriteFile(path, data, 0o600); err != nil {
		t.Fatal(err)
	}
	f, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	wantOffset := int64(len(data))
	if _, err := f.Seek(wantOffset, 0); err != nil {
		t.Fatal(err)
	}
	checkpoint, err := readEventGenerationCheckpoint(f, 0)
	if err != nil {
		t.Fatal(err)
	}
	if len(checkpoint.data) != eventGenerationAnchorLimit {
		t.Fatalf("checkpoint length=%d want=%d", len(checkpoint.data), eventGenerationAnchorLimit)
	}
	gotOffset, err := f.Seek(0, io.SeekCurrent)
	if err != nil {
		t.Fatal(err)
	}
	if gotOffset != wantOffset {
		t.Fatalf("checkpoint read moved sequential offset to %d want %d", gotOffset, wantOffset)
	}
}
