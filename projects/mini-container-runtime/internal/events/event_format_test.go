package events

import (
	"strings"
	"testing"
	"time"
)

func TestFormatEventPreservesLegacyShapeWithoutStructuredFields(t *testing.T) {
	evt := Event{
		Timestamp:   time.Date(2026, 9, 1, 1, 2, 3, 0, time.UTC),
		Type:        EventStart,
		ContainerID: "0123456789abcdef",
		Message:     "started container",
	}
	got := FormatEvent(evt)
	want := "2026-09-01T01:02:03Z container start 0123456789ab (started container)"
	if got != want {
		t.Fatalf("FormatEvent() = %q, want %q", got, want)
	}
}

func TestFormatEventSurfacesExecGenerationCommandAndZeroExit(t *testing.T) {
	zero := 0
	evt := Event{
		Timestamp:             time.Date(2026, 9, 1, 1, 2, 3, 0, time.UTC),
		Type:                  EventType("exec_exit"),
		ContainerID:           "abcdef0123456789",
		ContainerPID:          4242,
		ContainerPIDStartTime: 987654,
		Command:               []string{"/bin/sh", "-c", "printf '%s' 'a b'"},
		ExitCode:              &zero,
		Message:               "exec exited with code 0",
	}
	got := FormatEvent(evt)
	for _, want := range []string{
		"container exec_exit abcdef012345",
		"pid=4242",
		"pid_start=987654",
		`command=["/bin/sh","-c","printf '%s' 'a b'"]`,
		"exit_code=0",
		"(exec exited with code 0)",
	} {
		if !strings.Contains(got, want) {
			t.Fatalf("FormatEvent() = %q, missing %q", got, want)
		}
	}
}

func TestFormatEventQuotesStructuredErrorAsSingleAttribute(t *testing.T) {
	evt := Event{
		Timestamp:   time.Date(2026, 9, 1, 1, 2, 3, 0, time.UTC),
		Type:        EventType("exec_failed"),
		ContainerID: "deadbeef",
		Error:       "permission denied\nsecond line",
	}
	got := FormatEvent(evt)
	if !strings.Contains(got, `error="permission denied\nsecond line"`) {
		t.Fatalf("FormatEvent() did not quote error safely: %q", got)
	}
	if strings.Contains(got, "permission denied\nsecond line") {
		t.Fatalf("FormatEvent() emitted a literal newline inside one event record: %q", got)
	}
}

func TestFormatEventOmitsUnprovenGenerationFields(t *testing.T) {
	evt := Event{
		Timestamp:   time.Date(2026, 9, 1, 1, 2, 3, 0, time.UTC),
		Type:        EventExec,
		ContainerID: "deadbeef",
		Command:     []string{"true"},
	}
	got := FormatEvent(evt)
	if strings.Contains(got, "pid=") || strings.Contains(got, "pid_start=") {
		t.Fatalf("FormatEvent() fabricated generation attribution: %q", got)
	}
}
