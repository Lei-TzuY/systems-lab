package events

import (
	"encoding/json"
	"strings"
	"testing"
	"time"
)

func TestEventMarshalRejectsSemanticallyInvalidRecord(t *testing.T) {
	tests := []struct {
		name string
		evt  Event
		want string
	}{
		{
			name: "missing timestamp",
			evt:  Event{Type: EventStart, ContainerID: "abc123"},
			want: "missing timestamp",
		},
		{
			name: "missing container id",
			evt:  Event{Timestamp: time.Unix(1, 0).UTC(), Type: EventStart},
			want: "missing container_id",
		},
		{
			name: "unknown type",
			evt:  Event{Timestamp: time.Unix(1, 0).UTC(), Type: EventType("corrupt"), ContainerID: "abc123"},
			want: "unknown type",
		},
		{
			name: "incomplete process generation",
			evt: Event{
				Timestamp:    time.Unix(1, 0).UTC(),
				Type:         EventExec,
				ContainerID:  "abc123",
				ContainerPID: 42,
			},
			want: "incomplete container process generation",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, err := json.Marshal(tt.evt)
			if err == nil || !strings.Contains(err.Error(), tt.want) {
				t.Fatalf("json.Marshal() error = %v, want error containing %q", err, tt.want)
			}
		})
	}
}

func TestEventMarshalPreservesValidSchema(t *testing.T) {
	exitCode := 0
	evt := Event{
		Timestamp:             time.Unix(1, 0).UTC(),
		Type:                  EventExecExit,
		ContainerID:           "abcdef123456",
		ContainerPID:          42,
		ContainerPIDStartTime: 99,
		Command:               []string{"true"},
		ExitCode:              &exitCode,
	}

	data, err := json.Marshal(evt)
	if err != nil {
		t.Fatalf("json.Marshal(valid event): %v", err)
	}

	var got map[string]any
	if err := json.Unmarshal(data, &got); err != nil {
		t.Fatalf("json.Unmarshal(valid event): %v", err)
	}
	if got["type"] != string(EventExecExit) || got["container_id"] != evt.ContainerID {
		t.Fatalf("unexpected identity fields: %s", data)
	}
	if got["exit_code"] != float64(0) {
		t.Fatalf("exit_code lost or changed: %s", data)
	}
}
