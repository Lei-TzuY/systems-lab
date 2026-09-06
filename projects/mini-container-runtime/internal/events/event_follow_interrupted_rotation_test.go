package events

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestOpenEventLogGenerationForFollowFallsBackToRetainedOnlyAtStartup(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	retainedPath := path + ".1"
	if err := os.WriteFile(retainedPath, []byte("retained\n"), 0o600); err != nil {
		t.Fatal(err)
	}

	var opened []string
	f, expired, err := openEventLogGenerationForFollowWith(path, time.Time{}, true, func(candidate string) (*os.File, error) {
		opened = append(opened, candidate)
		return os.Open(candidate)
	}, time.Now, func(time.Duration) {
		t.Fatal("unexpected wait while retained generation exists")
	})
	if err != nil {
		t.Fatal(err)
	}
	if expired {
		t.Fatal("retained generation must be drained before follow expires")
	}
	defer f.Close()
	if len(opened) != 2 || opened[0] != path || opened[1] != retainedPath {
		t.Fatalf("open sequence=%q, want active then retained", opened)
	}

	calls := 0
	_, expired, err = openEventLogGenerationForFollowWith(path, time.Unix(1, 0).UTC(), false, func(candidate string) (*os.File, error) {
		calls++
		if candidate != path {
			t.Fatalf("post-attach reopen attempted retained path %q", candidate)
		}
		return nil, os.ErrNotExist
	}, func() time.Time { return time.Unix(1, 0).UTC() }, func(time.Duration) {
		t.Fatal("expired reopen must not wait")
	})
	if err != nil {
		t.Fatal(err)
	}
	if !expired || calls != 1 {
		t.Fatalf("expired=%v calls=%d, want true/1", expired, calls)
	}
}

func TestInterruptedRotationDrainsRetainedThenActiveExactlyOnce(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	retained := Event{Timestamp: time.Unix(1, 0).UTC(), Type: EventStop, ContainerID: "retained-once"}
	writeFollowTestRecord(t, path+".1", retained, true)

	f, expired, err := openEventLogGenerationForFollowWith(path, time.Time{}, true, openEventLogForRead, time.Now, func(time.Duration) {
		t.Fatal("unexpected wait while retained generation exists")
	})
	if err != nil || expired {
		t.Fatalf("open retained: expired=%v err=%v", expired, err)
	}

	var out bytes.Buffer
	reopen, err := followOpenEventLog(f, path, StreamOptions{Follow: true, JSON: true, Until: time.Unix(10, 0).UTC()}, &out)
	if closeErr := f.Close(); err == nil && closeErr != nil {
		err = closeErr
	}
	if err != nil {
		t.Fatalf("drain retained generation: %v", err)
	}
	if !reopen {
		t.Fatal("missing active pathname must request reopen after retained drain")
	}

	active := Event{Timestamp: time.Unix(2, 0).UTC(), Type: EventStart, ContainerID: "recovered-active"}
	writeFollowTestRecord(t, path, active, true)
	f, expired, err = openEventLogGenerationForFollowWith(path, time.Unix(10, 0).UTC(), false, openEventLogForRead, func() time.Time { return time.Unix(3, 0).UTC() }, func(time.Duration) {
		t.Fatal("active generation exists; reopen must not wait")
	})
	if err != nil || expired {
		t.Fatalf("open recovered active: expired=%v err=%v", expired, err)
	}
	reopen, err = followOpenEventLog(f, path, StreamOptions{Follow: true, JSON: true, Until: time.Unix(10, 0).UTC()}, &out)
	if closeErr := f.Close(); err == nil && closeErr != nil {
		err = closeErr
	}
	if err != nil {
		t.Fatalf("drain recovered active: %v", err)
	}
	if reopen {
		t.Fatal("stable recovered active generation unexpectedly requested reopen")
	}

	got := out.String()
	if strings.Count(got, "retained-once") != 1 || strings.Count(got, "recovered-active") != 1 {
		t.Fatalf("generation output=%q, want each event exactly once", got)
	}
}

func TestInterruptedRotationRetainedOpenFailsClosed(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	target := filepath.Join(dir, "target")
	if err := os.WriteFile(target, []byte("secret"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(target, path+".1"); err != nil {
		t.Fatal(err)
	}

	_, _, err := openEventLogGenerationForFollowWith(path, time.Now().Add(time.Second), true, openEventLogForRead, time.Now, func(time.Duration) {})
	if err == nil {
		t.Fatal("symlinked retained generation must fail closed")
	}
	if errors.Is(err, os.ErrNotExist) {
		t.Fatalf("unsafe retained generation was treated as absent: %v", err)
	}
}
