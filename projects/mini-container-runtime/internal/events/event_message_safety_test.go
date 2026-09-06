package events

import (
	"strings"
	"testing"
	"time"
)

func TestFormatEventEscapesNewlineInMessage(t *testing.T) {
	evt := Event{
		Timestamp:   time.Date(2026, 9, 1, 12, 0, 0, 0, time.UTC),
		Type:        EventStart,
		ContainerID: "abcdef0123456789",
		Message:     "started\n2026-09-01T12:00:01Z container die forged",
	}

	got := FormatEvent(evt)
	if strings.ContainsRune(got, '\n') {
		t.Fatalf("human event renderer emitted literal newline from message: %q", got)
	}
	if !strings.Contains(got, `("started\n2026-09-01T12:00:01Z container die forged")`) {
		t.Fatalf("control-bearing message was not safely quoted: %q", got)
	}
}

func TestFormatEventEscapesTerminalControlSequenceInMessage(t *testing.T) {
	evt := Event{
		Timestamp:   time.Date(2026, 9, 1, 12, 0, 0, 0, time.UTC),
		Type:        EventStop,
		ContainerID: "abcdef0123456789",
		Message:     "stopped\x1b[2Jforged",
	}

	got := FormatEvent(evt)
	if strings.ContainsRune(got, '\x1b') {
		t.Fatalf("human event renderer emitted literal terminal escape: %q", got)
	}
	if !strings.Contains(got, `("stopped\x1b[2Jforged")`) {
		t.Fatalf("terminal control sequence was not safely quoted: %q", got)
	}
}

func TestFormatEventPreservesPrintableMessageShape(t *testing.T) {
	evt := Event{
		Timestamp:   time.Date(2026, 9, 1, 12, 0, 0, 0, time.UTC),
		Type:        EventStart,
		ContainerID: "abcdef0123456789",
		Message:     "started container",
	}

	got := FormatEvent(evt)
	want := "2026-09-01T12:00:00Z container start abcdef012345 (started container)"
	if got != want {
		t.Fatalf("printable message compatibility changed: got %q want %q", got, want)
	}
}
