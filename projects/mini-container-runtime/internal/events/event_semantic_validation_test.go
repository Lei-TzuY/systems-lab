package events

import (
	"bytes"
	"strings"
	"testing"
	"time"
)

func TestStreamEventLogRejectsSemanticallyCorruptCompleteRecord(t *testing.T) {
	input := `{"timestamp":"2026-09-01T00:00:00Z","type":"start"}` + "\n"
	var out bytes.Buffer
	err := streamEventLogWithOptions(strings.NewReader(input), StreamOptions{}, &out)
	if err == nil || !strings.Contains(err.Error(), "validate event log: missing container_id") {
		t.Fatalf("error=%v, want semantic corruption rejection", err)
	}
	if out.Len() != 0 {
		t.Fatalf("semantically corrupt record produced output: %q", out.String())
	}
}

func TestStreamEventLogRejectsUnknownTypeEvenWhenFilterWouldHideIt(t *testing.T) {
	input := `{"timestamp":"2026-09-01T00:00:00Z","type":"future_typo","container_id":"abc123"}` + "\n"
	var out bytes.Buffer
	err := streamEventLogWithOptions(strings.NewReader(input), StreamOptions{Types: []EventType{EventStart}}, &out)
	if err == nil || !strings.Contains(err.Error(), `validate event log: unknown type "future_typo"`) {
		t.Fatalf("error=%v, want unknown-type corruption rejection", err)
	}
	if out.Len() != 0 {
		t.Fatalf("filter hid corruption but emitted output: %q", out.String())
	}
}

func TestStreamEventLogRejectsSemanticCorruptionAtEOFWithoutNewline(t *testing.T) {
	input := `{"timestamp":"2026-09-01T00:00:00Z","type":"start","container_id":""}`
	var out bytes.Buffer
	err := streamEventLogWithOptions(strings.NewReader(input), StreamOptions{}, &out)
	if err == nil || !strings.Contains(err.Error(), "validate event log: missing container_id") {
		t.Fatalf("error=%v, want complete semantic corruption rejected at EOF", err)
	}
}

func TestStreamEventLogStillIgnoresTornTrailingSyntax(t *testing.T) {
	valid := Event{Timestamp: time.Unix(1, 0).UTC(), Type: EventStart, ContainerID: "abc123"}
	input := eventRecordForTest(t, valid) + "\n" + `{"timestamp":"2026-09-01T00:00:00Z","type":"start"`
	var out bytes.Buffer
	if err := streamEventLogWithOptions(strings.NewReader(input), StreamOptions{}, &out); err != nil {
		t.Fatalf("torn trailing syntax should remain recoverable: %v", err)
	}
	if got, want := out.String(), FormatEvent(valid)+"\n"; got != want {
		t.Fatalf("output=%q, want durable prefix %q", got, want)
	}
}

func TestWriteCompleteEventRecordRejectsIncompleteProcessGeneration(t *testing.T) {
	line := []byte(`{"timestamp":"2026-09-01T00:00:00Z","type":"exec","container_id":"abc123","container_pid":42}` + "\n")
	var out bytes.Buffer
	err := writeCompleteEventRecord(line, StreamOptions{}, &out)
	if err == nil || !strings.Contains(err.Error(), "validate event log: incomplete container process generation") {
		t.Fatalf("error=%v, want generation corruption rejection", err)
	}
	if out.Len() != 0 {
		t.Fatalf("corrupt followed record produced output: %q", out.String())
	}
}
