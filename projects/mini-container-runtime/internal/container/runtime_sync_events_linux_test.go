//go:build linux

package container

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"testing"

	"minicontainer/internal/events"
)

func TestRuntimeSyncDoesNotCommitStartWhenReadyByteIsUndelivered(t *testing.T) {
	t.Setenv("HOME", t.TempDir())
	containerID := "sync-abort"
	if err := events.Publish(events.EventStart, containerID, "rootfs", "started container"); err != nil {
		t.Fatalf("stage start: %v", err)
	}

	readPipe, writePipe, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	defer readPipe.Close()
	if err := writePipe.Close(); err != nil {
		t.Fatal(err)
	}
	if err := releaseBlockedChild(writePipe); err == nil {
		t.Fatal("closed readiness writer reported success")
	}
	if err := events.Publish(events.EventDie, containerID, "rootfs", "exited with code 1"); err != nil {
		t.Fatalf("publish suppressed die: %v", err)
	}
	if _, err := os.Stat(events.LogPath()); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("undelivered readiness produced lifecycle log: err=%v", err)
	}
}

func TestRuntimeSyncReadyByteDoesNotCommitStartUntilPayloadExec(t *testing.T) {
	t.Setenv("HOME", t.TempDir())
	containerID := "sync-release"
	if err := events.Publish(events.EventStart, containerID, "rootfs", "started container"); err != nil {
		t.Fatalf("stage start: %v", err)
	}

	readPipe, writePipe, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	if err := releaseBlockedChild(writePipe); err != nil {
		t.Fatalf("release child: %v", err)
	}
	if err := awaitParentReady(readPipe); err != nil {
		t.Fatalf("await readiness: %v", err)
	}
	if _, err := os.Stat(events.LogPath()); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("parent readiness committed start before payload exec: err=%v", err)
	}

	initRead := writeInitStatusPipe(t, []byte{runtimeInitReadyByte})
	if err := awaitPayloadExec(initRead); err != nil {
		_ = initRead.Close()
		t.Fatalf("confirm payload exec: %v", err)
	}
	_ = initRead.Close()
	if err := events.Publish(events.EventDie, containerID, "rootfs", "exited with code 0"); err != nil {
		t.Fatalf("publish die: %v", err)
	}

	data, err := os.ReadFile(events.LogPath())
	if err != nil {
		t.Fatal(err)
	}
	dec := json.NewDecoder(bytes.NewReader(data))
	var got []events.Event
	for {
		var evt events.Event
		err := dec.Decode(&evt)
		if errors.Is(err, io.EOF) {
			break
		}
		if err != nil {
			t.Fatalf("decode lifecycle log: %v", err)
		}
		got = append(got, evt)
	}
	if len(got) != 2 || got[0].Type != events.EventStart || got[1].Type != events.EventDie {
		t.Fatalf("events=%+v, want start then die", got)
	}
	if got[0].ContainerID != containerID || got[1].ContainerID != containerID {
		t.Fatalf("events=%+v, want container ID %s", got, containerID)
	}
}
