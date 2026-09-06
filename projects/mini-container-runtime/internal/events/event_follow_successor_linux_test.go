//go:build linux

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

func TestOpenEventLogFollowSuccessorsReturnsAllGenerationsAfterPrevious(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	writeFollowTestRecord(t, path, Event{Timestamp: time.Unix(1, 0).UTC(), Type: EventStart, ContainerID: "old"}, true)
	previous, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer previous.Close()

	if err := os.Rename(path, path+".2"); err != nil {
		t.Fatal(err)
	}
	writeFollowTestRecord(t, path+".1", Event{Timestamp: time.Unix(2, 0).UTC(), Type: EventStop, ContainerID: "middle"}, true)
	writeFollowTestRecord(t, path, Event{Timestamp: time.Unix(3, 0).UTC(), Type: EventDie, ContainerID: "active"}, true)

	successors, err := openEventLogFollowSuccessors(path, previous)
	if err != nil {
		t.Fatalf("open successors: %v", err)
	}
	defer closeEventFollowFiles(successors)
	if len(successors) != 2 {
		t.Fatalf("successor count=%d, want 2", len(successors))
	}

	for i, want := range []string{"middle", "active"} {
		data, err := io.ReadAll(successors[i])
		if err != nil {
			t.Fatal(err)
		}
		if !bytes.Contains(data, []byte(want)) {
			t.Fatalf("successor %d=%q, want %q", i, data, want)
		}
	}
}

func TestFollowEventLogAttachmentDrainsTwoRapidRotationSuccessorsInOrder(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	writeFollowTestRecord(t, path, Event{Timestamp: time.Unix(1, 0).UTC(), Type: EventStart, ContainerID: "old"}, true)
	previous, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}

	if err := os.Rename(path, path+".2"); err != nil {
		t.Fatal(err)
	}
	writeFollowTestRecord(t, path+".1", Event{Timestamp: time.Unix(2, 0).UTC(), Type: EventStop, ContainerID: "middle"}, true)
	writeFollowTestRecord(t, path, Event{Timestamp: time.Unix(3, 0).UTC(), Type: EventDie, ContainerID: "active"}, true)

	var out bytes.Buffer
	reopen, err := followEventLogAttachment(previous, path, StreamOptions{JSON: true, Until: time.Now().Add(50 * time.Millisecond)}, &out)
	if err != nil {
		t.Fatalf("follow attachment: %v", err)
	}
	if reopen {
		t.Fatal("attachment unexpectedly requested another reopen")
	}

	got := out.String()
	oldAt := strings.Index(got, `"container_id":"old"`)
	middleAt := strings.Index(got, `"container_id":"middle"`)
	activeAt := strings.Index(got, `"container_id":"active"`)
	if oldAt < 0 || middleAt < 0 || activeAt < 0 {
		t.Fatalf("missing followed generation: %q", got)
	}
	if !(oldAt < middleAt && middleAt < activeAt) {
		t.Fatalf("generation order=%q, want old -> middle -> active", got)
	}
}

func TestOpenEventLogFollowSuccessorsReportsRetentionGap(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	writeFollowTestRecord(t, path, Event{Timestamp: time.Unix(1, 0).UTC(), Type: EventStart, ContainerID: "lost"}, true)
	previous, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer previous.Close()

	if err := os.Rename(path, path+".gone"); err != nil {
		t.Fatal(err)
	}
	writeFollowTestRecord(t, path+".2", Event{Timestamp: time.Unix(2, 0).UTC(), Type: EventStop, ContainerID: "retained-two"}, true)
	writeFollowTestRecord(t, path+".1", Event{Timestamp: time.Unix(3, 0).UTC(), Type: EventStop, ContainerID: "retained-one"}, true)
	writeFollowTestRecord(t, path, Event{Timestamp: time.Unix(4, 0).UTC(), Type: EventDie, ContainerID: "active"}, true)

	successors, err := openEventLogFollowSuccessors(path, previous)
	closeEventFollowFiles(successors)
	if err == nil || !strings.Contains(err.Error(), "event follow generation gap") {
		t.Fatalf("error=%v, want explicit retention gap", err)
	}
}

func TestOpenEventLogFollowSuccessorsReportsGapWhenWindowDisappears(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	writeFollowTestRecord(t, path, Event{Timestamp: time.Unix(1, 0).UTC(), Type: EventStart, ContainerID: "orphaned"}, true)
	previous, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer previous.Close()

	if err := os.Remove(path); err != nil {
		t.Fatal(err)
	}

	successors, err := openEventLogFollowSuccessors(path, previous)
	closeEventFollowFiles(successors)
	if err == nil || !strings.Contains(err.Error(), "event follow generation gap") {
		t.Fatalf("error=%v, want explicit gap after all managed generations disappear", err)
	}
}

func TestOpenEventLogFollowSuccessorsReopensCopytruncateActiveFromStart(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	writeFollowTestRecord(t, path, Event{Timestamp: time.Unix(1, 0).UTC(), Type: EventStart, ContainerID: "before"}, true)
	previous, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer previous.Close()

	writeFollowTestRecord(t, path, Event{Timestamp: time.Unix(2, 0).UTC(), Type: EventDie, ContainerID: "after"}, true)
	successors, err := openEventLogFollowSuccessors(path, previous)
	if err != nil {
		t.Fatalf("open copytruncate successor: %v", err)
	}
	defer closeEventFollowFiles(successors)
	if len(successors) != 1 {
		t.Fatalf("successor count=%d, want 1", len(successors))
	}
	data, err := io.ReadAll(successors[0])
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Contains(data, []byte("after")) {
		t.Fatalf("active successor=%q, want rewritten generation", data)
	}
}
