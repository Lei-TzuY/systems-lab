package events

import (
	"bytes"
	"strings"
	"testing"
)

func TestStreamEventLogRejectsDuplicateSecurityRelevantField(t *testing.T) {
	input := `{"timestamp":"2026-09-01T00:00:00Z","type":"start","type":"die","container_id":"abcdef0123456789"}` + "\n"

	var out bytes.Buffer
	err := streamEventLogWithOptions(strings.NewReader(input), StreamOptions{}, &out)
	if err == nil {
		t.Fatal("expected duplicate audit field to be rejected")
	}
	if !strings.Contains(err.Error(), `duplicate field "type"`) {
		t.Fatalf("unexpected error: %v", err)
	}
	if out.Len() != 0 {
		t.Fatalf("corrupt record must not be emitted, got %q", out.String())
	}
}

func TestStreamEventLogFilterCannotHideDuplicateFieldCorruption(t *testing.T) {
	input := `{"timestamp":"2026-09-01T00:00:00Z","type":"start","container_id":"abcdef0123456789","container_id":"ffffffffffffffff"}` + "\n"

	var out bytes.Buffer
	err := streamEventLogWithOptions(strings.NewReader(input), StreamOptions{
		ContainerPrefix: "does-not-match",
	}, &out)
	if err == nil {
		t.Fatal("expected duplicate field corruption to fail before filtering")
	}
	if !strings.Contains(err.Error(), `duplicate field "container_id"`) {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestStreamEventLogStillIgnoresTornDuplicateTail(t *testing.T) {
	valid := `{"timestamp":"2026-09-01T00:00:00Z","type":"start","container_id":"abcdef0123456789"}` + "\n"
	torn := `{"timestamp":"2026-09-01T00:00:01Z","type":"die","type":"start"`

	var out bytes.Buffer
	if err := streamEventLogWithOptions(strings.NewReader(valid+torn), StreamOptions{}, &out); err != nil {
		t.Fatalf("torn trailing record should remain recoverable: %v", err)
	}
	if !strings.Contains(out.String(), "container start abcdef012345") {
		t.Fatalf("expected durable record before torn tail, got %q", out.String())
	}
}

func TestRejectDuplicateTopLevelFieldsAllowsDistinctEventFields(t *testing.T) {
	line := []byte(`{"timestamp":"2026-09-01T00:00:00Z","type":"exec","container_id":"abcdef0123456789","command":["sh","-c","echo ok"],"message":"started"}`)
	if err := rejectDuplicateTopLevelFields(line); err != nil {
		t.Fatalf("valid audit object rejected: %v", err)
	}
}
