//go:build linux

package events

import (
	"bytes"
	"encoding/json"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func writeHistoryGeneration(t *testing.T, path string, evts ...Event) {
	t.Helper()
	var data bytes.Buffer
	for _, evt := range evts {
		encoded, err := json.Marshal(evt)
		if err != nil {
			t.Fatal(err)
		}
		data.Write(encoded)
		data.WriteByte('\n')
	}
	if err := os.WriteFile(path, data.Bytes(), 0o600); err != nil {
		t.Fatal(err)
	}
}

func TestHistoricalEventStreamReadsRetainedBeforeActive(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	old := Event{Timestamp: time.Unix(10, 0), Type: EventStart, ContainerID: "old-generation"}
	current := Event{Timestamp: time.Unix(20, 0), Type: EventDie, ContainerID: "current-generation"}
	writeHistoryGeneration(t, path+".1", old)
	writeHistoryGeneration(t, path, current)

	var out bytes.Buffer
	if err := streamHistoricalEventLogs(path, StreamOptions{JSON: true}, &out); err != nil {
		t.Fatalf("stream retained history: %v", err)
	}
	lines := strings.Split(strings.TrimSpace(out.String()), "\n")
	if len(lines) != 2 {
		t.Fatalf("lines=%d output=%q", len(lines), out.String())
	}
	var gotOld, gotCurrent Event
	if err := json.Unmarshal([]byte(lines[0]), &gotOld); err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal([]byte(lines[1]), &gotCurrent); err != nil {
		t.Fatal(err)
	}
	if gotOld.ContainerID != old.ContainerID || gotCurrent.ContainerID != current.ContainerID {
		t.Fatalf("generation order=%q, %q", gotOld.ContainerID, gotCurrent.ContainerID)
	}
}

func TestHistoricalEventStreamFiltersAcrossRetainedBoundary(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	writeHistoryGeneration(t, path+".1",
		Event{Timestamp: time.Unix(9, 0), Type: EventStart, ContainerID: "before"},
		Event{Timestamp: time.Unix(10, 0), Type: EventStart, ContainerID: "lower"},
	)
	writeHistoryGeneration(t, path,
		Event{Timestamp: time.Unix(20, 0), Type: EventDie, ContainerID: "upper"},
		Event{Timestamp: time.Unix(21, 0), Type: EventDie, ContainerID: "after"},
	)

	var out bytes.Buffer
	err := streamHistoricalEventLogs(path, StreamOptions{
		JSON:  true,
		Since: time.Unix(10, 0),
		Until: time.Unix(20, 0),
	}, &out)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(out.String(), "before") || strings.Contains(out.String(), "after") {
		t.Fatalf("time filter leaked out-of-range event: %q", out.String())
	}
	if !strings.Contains(out.String(), "lower") || !strings.Contains(out.String(), "upper") {
		t.Fatalf("inclusive boundary missing: %q", out.String())
	}
}

func TestHistoricalSnapshotDoesNotReadPostSnapshotAppend(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	first := Event{Timestamp: time.Unix(10, 0), Type: EventStart, ContainerID: "captured"}
	second := Event{Timestamp: time.Unix(11, 0), Type: EventDie, ContainerID: "late"}
	writeHistoryGeneration(t, path, first)

	snapshot, err := openEventLogSnapshotForRead(path)
	if err != nil {
		t.Fatal(err)
	}
	defer func() {
		for _, generation := range snapshot {
			_ = generation.file.Close()
		}
	}()

	encoded, err := json.Marshal(second)
	if err != nil {
		t.Fatal(err)
	}
	f, err := os.OpenFile(path, os.O_WRONLY|os.O_APPEND, 0)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := f.Write(append(encoded, '\n')); err != nil {
		_ = f.Close()
		t.Fatal(err)
	}
	if err := f.Close(); err != nil {
		t.Fatal(err)
	}

	var out bytes.Buffer
	for _, generation := range snapshot {
		if err := streamEventLogWithOptions(io.LimitReader(generation.file, generation.size), StreamOptions{JSON: true}, &out); err != nil {
			t.Fatal(err)
		}
	}
	if !strings.Contains(out.String(), "captured") || strings.Contains(out.String(), "late") {
		t.Fatalf("snapshot output=%q", out.String())
	}
}

func TestHistoricalSnapshotRejectsSymlinkRetainedGeneration(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	victim := filepath.Join(dir, "victim")
	writeHistoryGeneration(t, victim, Event{Timestamp: time.Unix(1, 0), Type: EventStart, ContainerID: "victim"})
	if err := os.Symlink(victim, path+".1"); err != nil {
		t.Fatal(err)
	}
	writeHistoryGeneration(t, path, Event{Timestamp: time.Unix(2, 0), Type: EventStart, ContainerID: "active"})

	var out bytes.Buffer
	if err := streamHistoricalEventLogs(path, StreamOptions{JSON: true}, &out); err == nil {
		t.Fatal("expected unsafe retained generation to fail closed")
	}
	if out.Len() != 0 {
		t.Fatalf("unsafe retained generation produced output: %q", out.String())
	}
}
