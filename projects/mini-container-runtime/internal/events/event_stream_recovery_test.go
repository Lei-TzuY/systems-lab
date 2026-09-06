package events

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
	"time"
)

func eventRecordForTest(t *testing.T, evt Event) string {
	t.Helper()
	data, err := json.Marshal(evt)
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}

func TestStreamEventLogIgnoresTornTrailingRecordButKeepsDurableHistory(t *testing.T) {
	first := Event{Timestamp: time.Unix(1, 0).UTC(), Type: EventStart, ContainerID: "0123456789abcdef", Message: "started"}
	second := Event{Timestamp: time.Unix(2, 0).UTC(), Type: EventDie, ContainerID: first.ContainerID, Message: "exited"}
	input := eventRecordForTest(t, first) + "\n" + eventRecordForTest(t, second)[:20]

	var out bytes.Buffer
	if err := streamEventLog(strings.NewReader(input), false, &out); err != nil {
		t.Fatalf("streamEventLog torn tail: %v", err)
	}
	if got, want := out.String(), FormatEvent(first)+"\n"; got != want {
		t.Fatalf("output=%q, want durable prefix %q", got, want)
	}
}

func TestStreamEventLogRejectsMalformedCompleteRecord(t *testing.T) {
	first := Event{Timestamp: time.Unix(1, 0).UTC(), Type: EventStart, ContainerID: "0123456789abcdef"}
	input := eventRecordForTest(t, first) + "\n{not-json}\n"

	var out bytes.Buffer
	err := streamEventLog(strings.NewReader(input), false, &out)
	if err == nil || !strings.Contains(err.Error(), "decode event log") {
		t.Fatalf("error=%v, want complete-record corruption", err)
	}
	if got, want := out.String(), FormatEvent(first)+"\n"; got != want {
		t.Fatalf("durable prefix output=%q, want %q", got, want)
	}
}

func TestStreamEventLogAcceptsValidFinalRecordWithoutNewline(t *testing.T) {
	evt := Event{Timestamp: time.Unix(3, 0).UTC(), Type: EventExec, ContainerID: "fedcba9876543210", Command: []string{"echo", "ok"}}

	var out bytes.Buffer
	if err := streamEventLog(strings.NewReader(eventRecordForTest(t, evt)), false, &out); err != nil {
		t.Fatalf("streamEventLog final record: %v", err)
	}
	if got, want := out.String(), FormatEvent(evt)+"\n"; got != want {
		t.Fatalf("output=%q, want %q", got, want)
	}
}
