package events

import (
	"encoding/json"
	"errors"
	"io"
	"os"
	"testing"
)

func readStructuredOutcomeEvent(t *testing.T) Event {
	t.Helper()
	f, err := os.Open(LogPath())
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	dec := json.NewDecoder(f)
	var last Event
	for {
		var evt Event
		if err := dec.Decode(&evt); err != nil {
			if errors.Is(err, io.EOF) {
				return last
			}
			t.Fatal(err)
		}
		last = evt
	}
}

func TestCompletePendingExecPublishesStructuredExitCode(t *testing.T) {
	resetExecStagingForTest(t)
	t.Setenv("HOME", t.TempDir())
	if err := Publish(EventExec, "ctr-structured-exit", "rootfs", "exec [false]"); err != nil {
		t.Fatal(err)
	}
	if err := CommitPendingExec(); err != nil {
		t.Fatal(err)
	}
	if err := CompletePendingExec(23, ""); err != nil {
		t.Fatal(err)
	}
	evt := readStructuredOutcomeEvent(t)
	if evt.Type != EventExecExit || evt.ExitCode == nil || *evt.ExitCode != 23 {
		t.Fatalf("structured exit outcome=%+v", evt)
	}
	if evt.Error != "" {
		t.Fatalf("unexpected structured error %q", evt.Error)
	}
}

func TestFailPendingExecPublishesStructuredErrorWithoutExitCode(t *testing.T) {
	resetExecStagingForTest(t)
	t.Setenv("HOME", t.TempDir())
	if err := Publish(EventExec, "ctr-structured-failure", "rootfs", "exec [missing]"); err != nil {
		t.Fatal(err)
	}
	const detail = "payload start was not proven"
	if err := FailPendingExec(detail); err != nil {
		t.Fatal(err)
	}
	evt := readStructuredOutcomeEvent(t)
	if evt.Type != EventExecFailed || evt.Error != detail {
		t.Fatalf("structured failure outcome=%+v", evt)
	}
	if evt.ExitCode != nil {
		t.Fatalf("pre-start failure unexpectedly has exit code %d", *evt.ExitCode)
	}
}
