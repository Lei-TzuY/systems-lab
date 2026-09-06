//go:build linux

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

func TestFollowStartupDrainsRetainedBeforeActiveExactlyOnce(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	retained := Event{Timestamp: time.Unix(1, 0).UTC(), Type: EventStop, ContainerID: "startup-retained"}
	active := Event{Timestamp: time.Unix(2, 0).UTC(), Type: EventStart, ContainerID: "startup-active"}
	writeFollowTestRecord(t, path+".1", retained, true)
	writeFollowTestRecord(t, path, active, true)

	var out bytes.Buffer
	if err := followEventLogFile(path, StreamOptions{Follow: true, JSON: true, Until: time.Unix(10, 0).UTC()}, &out); err != nil {
		t.Fatal(err)
	}
	got := out.String()
	if strings.Count(got, "startup-retained") != 1 || strings.Count(got, "startup-active") != 1 {
		t.Fatalf("output=%q, want retained and active exactly once", got)
	}
	if strings.Index(got, "startup-retained") > strings.Index(got, "startup-active") {
		t.Fatalf("output=%q, retained generation must precede active", got)
	}
}

func TestFollowStartupAppliesFiltersAcrossRetainedAndActive(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	writeFollowTestRecord(t, path+".1", Event{Timestamp: time.Unix(1, 0).UTC(), Type: EventStop, ContainerID: "keep-retained"}, true)
	writeFollowTestRecord(t, path, Event{Timestamp: time.Unix(2, 0).UTC(), Type: EventStart, ContainerID: "drop-active"}, true)

	var out bytes.Buffer
	opts := StreamOptions{Follow: true, JSON: true, Types: []EventType{EventStop}, Until: time.Unix(10, 0).UTC()}
	if err := followEventLogFile(path, opts, &out); err != nil {
		t.Fatal(err)
	}
	got := out.String()
	if strings.Count(got, "keep-retained") != 1 || strings.Contains(got, "drop-active") {
		t.Fatalf("filtered output=%q", got)
	}
}

func TestFollowStartupRejectsUnsafeRetainedEvenWhenActiveExists(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	writeFollowTestRecord(t, path, Event{Timestamp: time.Unix(2, 0).UTC(), Type: EventStart, ContainerID: "active"}, true)
	target := filepath.Join(dir, "target")
	if err := os.WriteFile(target, []byte("secret"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(target, path+".1"); err != nil {
		t.Fatal(err)
	}

	var out bytes.Buffer
	err := followEventLogFile(path, StreamOptions{Follow: true, JSON: true, Until: time.Unix(10, 0).UTC()}, &out)
	if err == nil {
		t.Fatal("unsafe retained generation must fail closed")
	}
	if errors.Is(err, os.ErrNotExist) {
		t.Fatalf("unsafe retained generation was treated as absent: %v", err)
	}
	if out.Len() != 0 {
		t.Fatalf("active event emitted before retained validation: %q", out.String())
	}
}
