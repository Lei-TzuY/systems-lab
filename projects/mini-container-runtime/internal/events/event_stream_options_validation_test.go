package events

import (
	"bytes"
	"strings"
	"testing"
)

func TestValidateStreamOptionsRejectsUnknownType(t *testing.T) {
	err := validateStreamOptions(StreamOptions{Types: []EventType{EventStart, EventType("strat")}})
	if err == nil {
		t.Fatal("expected unknown event type filter to be rejected")
	}
	if !strings.Contains(err.Error(), `unknown event type filter "strat"`) {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestValidateStreamOptionsRejectsEmptyExplicitType(t *testing.T) {
	err := validateStreamOptions(StreamOptions{Types: []EventType{""}})
	if err == nil {
		t.Fatal("expected empty explicit event type filter to be rejected")
	}
	if !strings.Contains(err.Error(), "event type filter must not be empty") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestValidateStreamOptionsAcceptsAllKnownTypes(t *testing.T) {
	types := []EventType{
		EventCreate,
		EventStart,
		EventExec,
		EventPause,
		EventUnpause,
		EventStop,
		EventSignal,
		EventDie,
		EventRemove,
		EventExecExit,
		EventExecFailed,
	}
	if err := validateStreamOptions(StreamOptions{Types: types}); err != nil {
		t.Fatalf("known event types rejected: %v", err)
	}
}

func TestStreamEventsWithOptionsRejectsInvalidTypeBeforeLogOpen(t *testing.T) {
	var out bytes.Buffer
	err := StreamEventsWithOptions(StreamOptions{Types: []EventType{EventType("bogus")}}, &out)
	if err == nil {
		t.Fatal("expected invalid stream options to fail")
	}
	if !strings.Contains(err.Error(), "invalid event stream options") {
		t.Fatalf("unexpected error: %v", err)
	}
	if out.Len() != 0 {
		t.Fatalf("invalid query emitted output: %q", out.String())
	}
}
