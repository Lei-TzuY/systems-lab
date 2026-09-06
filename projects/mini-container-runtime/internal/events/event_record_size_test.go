package events

import (
	"bytes"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestReadEventRecordAcceptsExactLimitWithNewline(t *testing.T) {
	input := strings.Repeat("x", maxEventRecordBytes) + "\n"
	reader := newEventRecordReader(strings.NewReader(input))

	line, err := readEventRecord(reader)
	if err != nil {
		t.Fatalf("read exact-limit record: %v", err)
	}
	if len(line) != maxEventRecordBytes+1 {
		t.Fatalf("record length=%d want=%d", len(line), maxEventRecordBytes+1)
	}
}

func TestReadEventRecordRejectsOversizedTerminatedAndTornRecords(t *testing.T) {
	for _, tc := range []struct {
		name  string
		suffix string
	}{
		{name: "terminated", suffix: "\n"},
		{name: "torn", suffix: ""},
	} {
		t.Run(tc.name, func(t *testing.T) {
			input := strings.Repeat("x", maxEventRecordBytes+1) + tc.suffix
			reader := newEventRecordReader(strings.NewReader(input))
			line, err := readEventRecord(reader)
			if err == nil || !strings.Contains(err.Error(), "exceeds maximum size") {
				t.Fatalf("error=%v, want oversized-record rejection", err)
			}
			if len(line) != 0 {
				t.Fatalf("oversized prefix leaked to caller: %d bytes", len(line))
			}
		})
	}
}

func TestHistoricalEventStreamRejectsOversizedRecordBeforeDecode(t *testing.T) {
	input := strings.Repeat("x", maxEventRecordBytes+1) + "\n"
	var out bytes.Buffer
	err := streamEventLogWithOptions(strings.NewReader(input), StreamOptions{}, &out)
	if err == nil || !strings.Contains(err.Error(), "exceeds maximum size") {
		t.Fatalf("error=%v, want oversized-record rejection", err)
	}
	if out.Len() != 0 {
		t.Fatalf("oversized record produced output: %q", out.String())
	}
}

func TestFollowEventStreamRejectsOversizedRecordWithoutWaiting(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	if err := os.WriteFile(path, []byte(strings.Repeat("x", maxEventRecordBytes+1)+"\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	f, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()

	var out bytes.Buffer
	reopen, err := followOpenEventLog(f, path, StreamOptions{Follow: true}, &out)
	if err == nil || !strings.Contains(err.Error(), "exceeds maximum size") {
		t.Fatalf("error=%v, want oversized-record rejection", err)
	}
	if reopen {
		t.Fatal("oversized current generation must fail closed, not request reopen")
	}
	if out.Len() != 0 {
		t.Fatalf("oversized followed record produced output: %q", out.String())
	}
}

func TestAppendEventRejectsOversizedRecordBeforeCreatingLog(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)

	evt := Event{
		Timestamp:   time.Unix(1, 0).UTC(),
		Type:        EventStop,
		ContainerID: "oversized-producer",
		Message:     strings.Repeat("x", maxEventRecordBytes),
	}
	err := appendEventUnlocked(evt)
	if err == nil || !strings.Contains(err.Error(), "exceeds maximum size") {
		t.Fatalf("error=%v, want producer-side size rejection", err)
	}
	if _, statErr := os.Stat(LogPath()); !os.IsNotExist(statErr) {
		t.Fatalf("oversized producer created event log: stat error=%v", statErr)
	}
}

func TestReadEventRecordPreservesEOFForBoundedTornTail(t *testing.T) {
	input := strings.Repeat("x", 128)
	reader := newEventRecordReader(strings.NewReader(input))
	line, err := readEventRecord(reader)
	if err != io.EOF {
		t.Fatalf("error=%v, want EOF", err)
	}
	if string(line) != input {
		t.Fatalf("bounded torn tail changed: got %q", string(line))
	}
}
