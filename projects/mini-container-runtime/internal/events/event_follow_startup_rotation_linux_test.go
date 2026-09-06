//go:build linux

package events

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"
)

type gatedCaptureWriter struct {
	mu      sync.Mutex
	buf     bytes.Buffer
	first   chan []byte
	release <-chan struct{}
	once    sync.Once
}

func (w *gatedCaptureWriter) Write(p []byte) (int, error) {
	copyOfP := append([]byte(nil), p...)
	blocked := false
	w.once.Do(func() {
		blocked = true
		w.first <- copyOfP
	})
	if blocked {
		<-w.release
	}
	w.mu.Lock()
	defer w.mu.Unlock()
	return w.buf.Write(p)
}

func (w *gatedCaptureWriter) String() string {
	w.mu.Lock()
	defer w.mu.Unlock()
	return w.buf.String()
}

func TestFollowStartupRotationDuringRetainedDrainHasNoGapOrReplay(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	retained := Event{Timestamp: time.Unix(1, 0).UTC(), Type: EventStop, ContainerID: "race-retained"}
	oldActive := Event{Timestamp: time.Unix(2, 0).UTC(), Type: EventStart, ContainerID: "race-old-active"}
	newActive := Event{Timestamp: time.Unix(3, 0).UTC(), Type: EventDie, ContainerID: "race-new-active"}
	writeFollowTestRecord(t, path+".1", retained, true)
	writeFollowTestRecord(t, path, oldActive, true)

	release := make(chan struct{})
	writer := &gatedCaptureWriter{first: make(chan []byte, 1), release: release}
	result := make(chan error, 1)
	go func() {
		result <- followEventLogFile(path, StreamOptions{
			Follow: true,
			JSON:   true,
			Until:  time.Now().Add(2 * time.Second),
		}, writer)
	}()

	select {
	case first := <-writer.first:
		if !bytes.Contains(first, []byte("race-retained")) {
			t.Fatalf("first startup record=%q, want retained generation", first)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("timed out waiting for retained startup record")
	}

	// The startup snapshot is already holding descriptors for both retained and
	// active generations. Rotate active while retained output is deliberately
	// blocked, replacing the retained pathname, then publish a new active
	// generation. The follower must drain the descriptors it captured before the
	// rotation, notice the active pathname replacement, and hand off to the new
	// generation without replaying the replacement .1 pathname.
	if err := os.Rename(path, path+".1"); err != nil {
		t.Fatalf("rotate active during startup drain: %v", err)
	}
	writeFollowTestRecord(t, path, newActive, true)
	close(release)

	select {
	case err := <-result:
		if err != nil {
			t.Fatalf("follow startup rotation race: %v", err)
		}
	case <-time.After(4 * time.Second):
		t.Fatal("follower did not terminate after --until")
	}

	got := writer.String()
	for _, id := range []string{"race-retained", "race-old-active", "race-new-active"} {
		if count := strings.Count(got, id); count != 1 {
			t.Fatalf("output=%q, %s count=%d want=1", got, id, count)
		}
	}
	if retainedPos, oldPos, newPos := strings.Index(got, "race-retained"), strings.Index(got, "race-old-active"), strings.Index(got, "race-new-active"); !(retainedPos < oldPos && oldPos < newPos) {
		t.Fatalf("output=%q, want retained -> old active -> new active ordering", got)
	}
}
