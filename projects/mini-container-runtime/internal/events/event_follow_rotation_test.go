package events

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func writeFollowTestRecord(t *testing.T, path string, evt Event, newline bool) {
	t.Helper()
	data, err := json.Marshal(evt)
	if err != nil {
		t.Fatal(err)
	}
	if newline {
		data = append(data, '\n')
	}
	if err := os.WriteFile(path, data, 0o600); err != nil {
		t.Fatal(err)
	}
}

func marshalFollowTestRecord(t *testing.T, evt Event) []byte {
	t.Helper()
	data, err := json.Marshal(evt)
	if err != nil {
		t.Fatal(err)
	}
	return append(data, '\n')
}

type notifyingWriter struct {
	writes  chan []byte
	release <-chan struct{}
}

func (w notifyingWriter) Write(p []byte) (int, error) {
	copyOfP := append([]byte(nil), p...)
	select {
	case w.writes <- copyOfP:
	default:
	}
	if w.release != nil {
		<-w.release
	}
	return len(p), nil
}

func TestFollowOpenEventLogRequestsReopenAfterPathReplacement(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	oldEvent := Event{Timestamp: time.Unix(1, 0).UTC(), Type: EventStart, ContainerID: "old-generation"}
	writeFollowTestRecord(t, path, oldEvent, true)

	f, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()

	rotated := filepath.Join(dir, "events.log.1")
	if err := os.Rename(path, rotated); err != nil {
		t.Fatal(err)
	}
	newEvent := Event{Timestamp: time.Unix(2, 0).UTC(), Type: EventDie, ContainerID: "new-generation"}
	writeFollowTestRecord(t, path, newEvent, true)

	var out bytes.Buffer
	reopen, err := followOpenEventLog(f, path, StreamOptions{JSON: true}, &out)
	if err != nil {
		t.Fatalf("follow old generation: %v", err)
	}
	if !reopen {
		t.Fatal("expected path replacement to request reopen")
	}
	got := out.String()
	if !strings.Contains(got, "old-generation") {
		t.Fatalf("durable old event was not emitted before reopen: %q", got)
	}
	if strings.Contains(got, "new-generation") {
		t.Fatalf("old descriptor unexpectedly observed replacement event: %q", got)
	}
}

func TestFollowOpenEventLogRequestsReopenWhenPathDisappears(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	oldEvent := Event{Timestamp: time.Unix(4, 0).UTC(), Type: EventStop, ContainerID: "unlinked-generation"}
	writeFollowTestRecord(t, path, oldEvent, true)

	f, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	if err := os.Remove(path); err != nil {
		t.Fatal(err)
	}

	var out bytes.Buffer
	reopen, err := followOpenEventLog(f, path, StreamOptions{JSON: true}, &out)
	if err != nil {
		t.Fatalf("follow unlinked generation: %v", err)
	}
	if !reopen {
		t.Fatal("expected missing logical path at EOF to request reopen")
	}
	if got := out.String(); !strings.Contains(got, "unlinked-generation") {
		t.Fatalf("durable pre-unlink event was not emitted before reopen: %q", got)
	}
}

func TestFollowOpenEventLogDropsTornOldTailOnReplacement(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	if err := os.WriteFile(path, []byte(`{"timestamp":"2026-01-01T00:00:00Z","type":"exec"`), 0o600); err != nil {
		t.Fatal(err)
	}
	f, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()

	if err := os.Rename(path, filepath.Join(dir, "events.log.1")); err != nil {
		t.Fatal(err)
	}
	writeFollowTestRecord(t, path, Event{Timestamp: time.Unix(3, 0).UTC(), Type: EventExec, ContainerID: "replacement"}, true)

	var out bytes.Buffer
	reopen, err := followOpenEventLog(f, path, StreamOptions{}, &out)
	if err != nil {
		t.Fatalf("torn old tail during replacement must be ignored: %v", err)
	}
	if !reopen {
		t.Fatal("expected replacement after torn tail to request reopen")
	}
	if out.Len() != 0 {
		t.Fatalf("torn old record was emitted: %q", out.String())
	}
}

func TestFollowOpenEventLogDetectsCopytruncateAfterFastRegrowth(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	oldEvent := Event{Timestamp: time.Unix(5, 0).UTC(), Type: EventStart, ContainerID: "copytruncate-old"}
	oldRecord := marshalFollowTestRecord(t, oldEvent)
	if err := os.WriteFile(path, oldRecord, 0o600); err != nil {
		t.Fatal(err)
	}

	f, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	before, err := f.Stat()
	if err != nil {
		t.Fatal(err)
	}

	writes := make(chan []byte, 1)
	release := make(chan struct{})
	released := false
	defer func() {
		if !released {
			close(release)
		}
	}()
	result := make(chan struct {
		reopen bool
		err    error
	}, 1)
	go func() {
		reopen, err := followOpenEventLog(f, path, StreamOptions{JSON: true}, notifyingWriter{writes: writes, release: release})
		result <- struct {
			reopen bool
			err    error
		}{reopen: reopen, err: err}
	}()

	select {
	case first := <-writes:
		if !bytes.Contains(first, []byte("copytruncate-old")) {
			t.Fatalf("first followed record=%q, want old generation", first)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("timed out waiting for old generation to reach follower")
	}

	newEvent := Event{
		Timestamp:   time.Unix(6, 0).UTC(),
		Type:        EventDie,
		ContainerID: "copytruncate-new",
		Message:     strings.Repeat("x", len(oldRecord)+256),
	}
	newRecord := marshalFollowTestRecord(t, newEvent)
	if len(newRecord) <= len(oldRecord) {
		t.Fatalf("test setup requires regrowth beyond old offset: old=%d new=%d", len(oldRecord), len(newRecord))
	}
	if err := os.WriteFile(path, newRecord, 0o600); err != nil {
		t.Fatal(err)
	}
	after, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if !os.SameFile(before, after) {
		t.Fatal("test setup replaced inode; expected copytruncate on same inode")
	}

	// Release the follower only after the same inode has been truncated and
	// regrown past its inherited read offset. This deterministically exercises
	// the window that previously depended on scheduler timing.
	close(release)
	released = true

	select {
	case got := <-result:
		if got.err != nil {
			t.Fatalf("follow copytruncate generation: %v", got.err)
		}
		if !got.reopen {
			t.Fatal("expected copytruncate generation reset to request reopen")
		}
	case <-time.After(2 * time.Second):
		t.Fatal("follower missed copytruncate after file regrew past old offset")
	}
}

func TestReadEventGenerationAnchorDoesNotMoveSequentialOffset(t *testing.T) {
	path := filepath.Join(t.TempDir(), "events.log")
	record := marshalFollowTestRecord(t, Event{Timestamp: time.Unix(7, 0).UTC(), Type: EventStop, ContainerID: "anchor"})
	if err := os.WriteFile(path, record, 0o600); err != nil {
		t.Fatal(err)
	}
	f, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	if _, err := f.Seek(3, 0); err != nil {
		t.Fatal(err)
	}
	anchor, err := readEventGenerationAnchor(f)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(anchor, record) {
		t.Fatalf("anchor=%q want=%q", anchor, record)
	}
	offset, err := f.Seek(0, 1)
	if err != nil {
		t.Fatal(err)
	}
	if offset != 3 {
		t.Fatalf("anchor read moved sequential offset to %d, want 3", offset)
	}
}

func TestWriteCompleteEventRecordStillFailsClosedOnMalformedCompleteRecord(t *testing.T) {
	var out bytes.Buffer
	err := writeCompleteEventRecord([]byte("{not-json}\n"), StreamOptions{}, &out)
	if err == nil || !strings.Contains(err.Error(), "decode event log") {
		t.Fatalf("error=%v, want complete-record corruption rejection", err)
	}
	if out.Len() != 0 {
		t.Fatalf("malformed record produced output: %q", out.String())
	}
}
