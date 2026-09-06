package events

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestFollowOpenEventLogUntilPastDrainsExistingAndStops(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	evt := Event{Timestamp: time.Unix(10, 0).UTC(), Type: EventStart, ContainerID: "deadline-existing"}
	writeFollowTestRecord(t, path, evt, true)

	f, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()

	var out bytes.Buffer
	reopen, err := followOpenEventLog(f, path, StreamOptions{
		Follow: true,
		JSON:   true,
		Until:  time.Unix(20, 0).UTC(),
	}, &out)
	if err != nil {
		t.Fatalf("follow through expired deadline: %v", err)
	}
	if reopen {
		t.Fatal("stable exhausted generation requested reopen after deadline")
	}
	if got := out.String(); !strings.Contains(got, "deadline-existing") {
		t.Fatalf("existing durable record was not drained before deadline exit: %q", got)
	}
}

func TestFollowOpenEventLogUntilPastDropsTornTail(t *testing.T) {
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

	var out bytes.Buffer
	reopen, err := followOpenEventLog(f, path, StreamOptions{Follow: true, Until: time.Unix(20, 0).UTC()}, &out)
	if err != nil {
		t.Fatalf("expired deadline with torn tail: %v", err)
	}
	if reopen {
		t.Fatal("stable torn-tail generation requested reopen")
	}
	if out.Len() != 0 {
		t.Fatalf("torn tail was emitted at deadline: %q", out.String())
	}
}

func TestOpenEventLogForFollowWithDeadlineBoundsMissingLogWait(t *testing.T) {
	start := time.Unix(100, 0).UTC()
	now := start
	until := start.Add(50 * time.Millisecond)
	calls := 0
	var waits []time.Duration

	f, expired, err := openEventLogForFollowWith("ignored", until, func(string) (*os.File, error) {
		calls++
		return nil, os.ErrNotExist
	}, func() time.Time {
		return now
	}, func(delay time.Duration) {
		waits = append(waits, delay)
		now = now.Add(delay)
	})
	if err != nil {
		t.Fatalf("open missing log through deadline: %v", err)
	}
	if f != nil {
		f.Close()
		t.Fatal("unexpectedly opened missing log")
	}
	if !expired {
		t.Fatal("missing log did not terminate at until deadline")
	}
	if calls != 2 {
		t.Fatalf("open calls=%d, want 2", calls)
	}
	if len(waits) != 1 || waits[0] != 50*time.Millisecond {
		t.Fatalf("waits=%v, want [50ms]", waits)
	}
}

func TestOpenEventLogForFollowWithExpiredDeadlineStillDrainsExistingFile(t *testing.T) {
	f, err := os.CreateTemp(t.TempDir(), "events-")
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()

	calls := 0
	got, expired, err := openEventLogForFollowWith("ignored", time.Unix(1, 0).UTC(), func(string) (*os.File, error) {
		calls++
		return f, nil
	}, func() time.Time {
		return time.Unix(2, 0).UTC()
	}, func(time.Duration) {
		t.Fatal("existing log must not wait")
	})
	if err != nil {
		t.Fatalf("open existing log after deadline: %v", err)
	}
	if expired {
		t.Fatal("deadline skipped an existing log that still needs draining")
	}
	if got != f {
		t.Fatalf("opened file=%v, want %v", got, f)
	}
	if calls != 1 {
		t.Fatalf("open calls=%d, want 1", calls)
	}
}
