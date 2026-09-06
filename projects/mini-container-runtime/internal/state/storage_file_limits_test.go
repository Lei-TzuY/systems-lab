package state

import (
	"os"
	"strings"
	"testing"
)

func TestReadBoundedStateFileAcceptsExactLimit(t *testing.T) {
	path := t.TempDir() + "/state.json"
	if err := os.WriteFile(path, make([]byte, maxStateFileBytes), 0o600); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}
	file, err := os.Open(path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer file.Close()

	data, err := readBoundedStateFile(file, maxStateFileBytes, "container state")
	if err != nil {
		t.Fatalf("readBoundedStateFile: %v", err)
	}
	if int64(len(data)) != maxStateFileBytes {
		t.Fatalf("len(data) = %d, want %d", len(data), maxStateFileBytes)
	}
}

func TestReadBoundedStateFileRejectsObservedOversize(t *testing.T) {
	path := t.TempDir() + "/state.json"
	file, err := os.Create(path)
	if err != nil {
		t.Fatalf("Create: %v", err)
	}
	if err := file.Truncate(maxStateFileBytes + 1); err != nil {
		file.Close()
		t.Fatalf("Truncate: %v", err)
	}
	if _, err := file.Seek(0, 0); err != nil {
		file.Close()
		t.Fatalf("Seek: %v", err)
	}
	defer file.Close()

	_, err = readBoundedStateFile(file, maxStateFileBytes+1, "container state")
	if err == nil || !strings.Contains(err.Error(), "size limit") {
		t.Fatalf("expected size-limit error, got %v", err)
	}
}

func TestReadBoundedStateFileRejectsGrowthPastObservedSize(t *testing.T) {
	path := t.TempDir() + "/state.json"
	if err := os.WriteFile(path, make([]byte, maxStateFileBytes+1), 0o600); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}
	file, err := os.Open(path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer file.Close()

	// Simulate a stale pre-read size observation. The bounded read itself must
	// still reject growth beyond the configured maximum.
	_, err = readBoundedStateFile(file, 0, "container state")
	if err == nil || !strings.Contains(err.Error(), "size limit") {
		t.Fatalf("expected size-limit error after growth, got %v", err)
	}
}
