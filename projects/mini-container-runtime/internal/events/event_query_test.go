package events

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
	"time"
)

func encodeQueryEvents(t *testing.T, events ...Event) string {
	t.Helper()
	var b strings.Builder
	for _, evt := range events {
		data, err := json.Marshal(evt)
		if err != nil {
			t.Fatal(err)
		}
		b.Write(data)
		b.WriteByte('\n')
	}
	return b.String()
}

func TestStreamEventLogWithOptionsFiltersContainerPrefixAndTypes(t *testing.T) {
	input := encodeQueryEvents(t,
		Event{Timestamp: time.Unix(1, 0), Type: EventStart, ContainerID: "abcdef0123456789"},
		Event{Timestamp: time.Unix(2, 0), Type: EventExec, ContainerID: "abcdef0123456789", Command: []string{"echo", "hello world"}},
		Event{Timestamp: time.Unix(3, 0), Type: EventExec, ContainerID: "ffff000012345678"},
		Event{Timestamp: time.Unix(4, 0), Type: EventDie, ContainerID: "abcdef0123456789"},
	)

	var out bytes.Buffer
	err := streamEventLogWithOptions(strings.NewReader(input), StreamOptions{
		ContainerPrefix: "abcdef",
		Types:           []EventType{EventExec},
	}, &out)
	if err != nil {
		t.Fatalf("stream query: %v", err)
	}
	got := out.String()
	if strings.Count(got, "\n") != 1 {
		t.Fatalf("filtered output=%q, want exactly one record", got)
	}
	if !strings.Contains(got, "container exec abcdef012345") || !strings.Contains(got, `command=["echo","hello world"]`) {
		t.Fatalf("filtered output=%q", got)
	}
	if strings.Contains(got, "ffff") || strings.Contains(got, "container start") || strings.Contains(got, "container die") {
		t.Fatalf("query leaked unmatched event: %q", got)
	}
}

func TestStreamEventLogWithOptionsJSONPreservesStructuredZeroExitCode(t *testing.T) {
	zero := 0
	want := Event{
		Timestamp:             time.Unix(10, 123),
		Type:                  EventExecExit,
		ContainerID:           "0123456789abcdef",
		ContainerPID:          42,
		ContainerPIDStartTime: 99,
		Command:               []string{"/bin/true"},
		ExitCode:              &zero,
	}
	input := encodeQueryEvents(t, want)

	var out bytes.Buffer
	if err := streamEventLogWithOptions(strings.NewReader(input), StreamOptions{JSON: true}, &out); err != nil {
		t.Fatalf("stream json: %v", err)
	}
	lines := strings.Split(strings.TrimSpace(out.String()), "\n")
	if len(lines) != 1 {
		t.Fatalf("JSON output lines=%d: %q", len(lines), out.String())
	}
	var got Event
	if err := json.Unmarshal([]byte(lines[0]), &got); err != nil {
		t.Fatalf("decode JSON output: %v", err)
	}
	if got.Type != want.Type || got.ContainerID != want.ContainerID || got.ExitCode == nil || *got.ExitCode != 0 || got.ContainerPID != 42 || got.ContainerPIDStartTime != 99 {
		t.Fatalf("JSON event=%+v, want structured event %+v", got, want)
	}
}

func TestStreamEventLogWithOptionsDoesNotHideCompleteCorruptionBehindFilter(t *testing.T) {
	valid := encodeQueryEvents(t, Event{Timestamp: time.Unix(1, 0), Type: EventStart, ContainerID: "target"})
	input := valid + "{not-json}\n"

	var out bytes.Buffer
	err := streamEventLogWithOptions(strings.NewReader(input), StreamOptions{ContainerPrefix: "does-not-match"}, &out)
	if err == nil || !strings.Contains(err.Error(), "decode event log") {
		t.Fatalf("error=%v, want complete corruption rejection", err)
	}
	if out.Len() != 0 {
		t.Fatalf("unmatched valid record should not be rendered: %q", out.String())
	}
}

func TestEventMatchesQueryUsesORAcrossTypesAndExactPrefixSemantics(t *testing.T) {
	evt := Event{Type: EventExecFailed, ContainerID: "abc123"}
	if !eventMatchesQuery(evt, StreamOptions{ContainerPrefix: "abc", Types: []EventType{EventExec, EventExecFailed}}) {
		t.Fatal("matching prefix/type OR query rejected event")
	}
	if eventMatchesQuery(evt, StreamOptions{ContainerPrefix: "bc"}) {
		t.Fatal("non-prefix substring matched container selector")
	}
	if eventMatchesQuery(evt, StreamOptions{Types: []EventType{EventExec}}) {
		t.Fatal("unmatched type accepted")
	}
}
