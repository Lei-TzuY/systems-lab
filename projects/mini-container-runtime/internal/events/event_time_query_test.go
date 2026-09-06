package events

import (
	"bytes"
	"strings"
	"testing"
	"time"
)

func TestEventMatchesQueryTimeBoundsAreInclusive(t *testing.T) {
	since := time.Unix(10, 100)
	until := time.Unix(20, 200)

	for _, tc := range []struct {
		name      string
		timestamp time.Time
		want      bool
	}{
		{name: "before", timestamp: since.Add(-time.Nanosecond), want: false},
		{name: "since-boundary", timestamp: since, want: true},
		{name: "inside", timestamp: time.Unix(15, 0), want: true},
		{name: "until-boundary", timestamp: until, want: true},
		{name: "after", timestamp: until.Add(time.Nanosecond), want: false},
	} {
		t.Run(tc.name, func(t *testing.T) {
			evt := Event{Timestamp: tc.timestamp, Type: EventStart, ContainerID: "abc"}
			got := eventMatchesQuery(evt, StreamOptions{Since: since, Until: until})
			if got != tc.want {
				t.Fatalf("match = %v, want %v", got, tc.want)
			}
		})
	}
}

func TestStreamEventLogWithOptionsFiltersInclusiveTimeWindow(t *testing.T) {
	input := encodeQueryEvents(t,
		Event{Timestamp: time.Unix(9, 0), Type: EventStart, ContainerID: "before"},
		Event{Timestamp: time.Unix(10, 0), Type: EventStart, ContainerID: "since"},
		Event{Timestamp: time.Unix(15, 0), Type: EventStart, ContainerID: "inside"},
		Event{Timestamp: time.Unix(20, 0), Type: EventStart, ContainerID: "until"},
		Event{Timestamp: time.Unix(21, 0), Type: EventStart, ContainerID: "after"},
	)

	var out bytes.Buffer
	if err := streamEventLogWithOptions(strings.NewReader(input), StreamOptions{
		Since: time.Unix(10, 0),
		Until: time.Unix(20, 0),
	}, &out); err != nil {
		t.Fatalf("stream time query: %v", err)
	}
	got := out.String()
	for _, want := range []string{"since", "inside", "until"} {
		if !strings.Contains(got, want) {
			t.Fatalf("output %q missing %q", got, want)
		}
	}
	for _, unwanted := range []string{"before", "after"} {
		if strings.Contains(got, unwanted) {
			t.Fatalf("output %q leaked %q", got, unwanted)
		}
	}
}

func TestValidateStreamOptionsRejectsReversedTimeWindow(t *testing.T) {
	err := validateStreamOptions(StreamOptions{
		Since: time.Unix(20, 0),
		Until: time.Unix(10, 0),
	})
	if err == nil || !strings.Contains(err.Error(), "must not be after") {
		t.Fatalf("error = %v", err)
	}
}

func TestTimeFilterDoesNotHideCompleteCorruption(t *testing.T) {
	valid := encodeQueryEvents(t, Event{Timestamp: time.Unix(1, 0), Type: EventStart, ContainerID: "old"})
	input := valid + "{not-json}\n"

	var out bytes.Buffer
	err := streamEventLogWithOptions(strings.NewReader(input), StreamOptions{Since: time.Unix(100, 0)}, &out)
	if err == nil || !strings.Contains(err.Error(), "decode event log") {
		t.Fatalf("error=%v, want corruption rejection", err)
	}
	if out.Len() != 0 {
		t.Fatalf("filtered record should not be rendered: %q", out.String())
	}
}
