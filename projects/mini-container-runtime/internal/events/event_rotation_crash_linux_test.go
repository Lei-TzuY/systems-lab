//go:build linux

package events

import (
	"bytes"
	"io"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestRetainedRotationIntermediateStatePreservesHistory(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	writeFollowTestRecord(t, path+".1", Event{
		Timestamp:   time.Unix(1, 0).UTC(),
		Type:        EventStop,
		ContainerID: "retained-before-shift",
	}, true)
	writeFollowTestRecord(t, path, Event{
		Timestamp:   time.Unix(2, 0).UTC(),
		Type:        EventStart,
		ContainerID: "active-before-shift",
	}, true)

	// rotateRetainedEventLog is the first mutating step of a full rotation. It
	// must return only after `.1 -> .2` is directory-durable, because a crash can
	// occur before the subsequent active -> `.1` rename.
	if err := rotateRetainedEventLog(path); err != nil {
		t.Fatalf("rotate retained generation: %v", err)
	}
	if _, err := os.Lstat(path + ".1"); !os.IsNotExist(err) {
		t.Fatalf("newer retained path still exists after shift: %v", err)
	}
	if _, err := os.Lstat(path + ".2"); err != nil {
		t.Fatalf("older retained path missing after shift: %v", err)
	}
	if _, err := os.Lstat(path); err != nil {
		t.Fatalf("active path changed during retained-only shift: %v", err)
	}

	// This is exactly the on-disk namespace a crash can expose after the first
	// durable barrier. Historical readers must still see both generations in
	// chronological order, proving the intermediate state is recoverable.
	snapshot, err := openEventLogSnapshotForRead(path)
	if err != nil {
		t.Fatalf("open interrupted-rotation snapshot: %v", err)
	}
	defer func() {
		for _, generation := range snapshot {
			_ = generation.file.Close()
		}
	}()
	if len(snapshot) != 2 {
		t.Fatalf("snapshot generations=%d, want retained .2 plus active", len(snapshot))
	}

	for i, want := range []string{"retained-before-shift", "active-before-shift"} {
		data, err := io.ReadAll(io.LimitReader(snapshot[i].file, snapshot[i].size))
		if err != nil {
			t.Fatalf("read snapshot generation %d: %v", i, err)
		}
		if !bytes.Contains(data, []byte(want)) {
			t.Fatalf("snapshot generation %d=%q, want %q", i, data, want)
		}
	}
}
