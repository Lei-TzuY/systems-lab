package main

import (
	"bytes"
	"reflect"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/events"
)

func TestParseEventsCLIOptionsFollowAliases(t *testing.T) {
	for _, flagName := range []string{"-f", "--follow"} {
		t.Run(flagName, func(t *testing.T) {
			opts, err := parseEventsCLIOptions([]string{flagName}, &bytes.Buffer{})
			if err != nil {
				t.Fatalf("parse: %v", err)
			}
			if !opts.Follow {
				t.Fatalf("Follow = false for %s", flagName)
			}
		})
	}
}

func TestParseEventsCLIOptionsQuery(t *testing.T) {
	opts, err := parseEventsCLIOptions([]string{
		"--json",
		"--container", "deadbeef",
		"--type", "start",
		"--type=die",
		"--since", "2026-09-01T01:02:03.123456789Z",
		"--until=2026-09-01T09:10:11+08:00",
	}, &bytes.Buffer{})
	if err != nil {
		t.Fatalf("parse: %v", err)
	}
	if !opts.JSON {
		t.Fatal("JSON = false")
	}
	if opts.ContainerPrefix != "deadbeef" {
		t.Fatalf("ContainerPrefix = %q", opts.ContainerPrefix)
	}
	wantTypes := []events.EventType{events.EventStart, events.EventDie}
	if !reflect.DeepEqual(opts.Types, wantTypes) {
		t.Fatalf("Types = %#v, want %#v", opts.Types, wantTypes)
	}
	wantSince := time.Date(2026, 9, 1, 1, 2, 3, 123456789, time.UTC)
	if !opts.Since.Equal(wantSince) {
		t.Fatalf("Since = %s, want %s", opts.Since, wantSince)
	}
	wantUntil := time.Date(2026, 9, 1, 1, 10, 11, 0, time.UTC)
	if !opts.Until.Equal(wantUntil) {
		t.Fatalf("Until = %s, want %s", opts.Until, wantUntil)
	}
}

func TestParseEventsCLIOptionsRejectsMalformedTimeBounds(t *testing.T) {
	for _, tc := range []struct {
		name string
		args []string
	}{
		{name: "since", args: []string{"--since", "yesterday"}},
		{name: "until", args: []string{"--until", "2026-09-01"}},
	} {
		t.Run(tc.name, func(t *testing.T) {
			_, err := parseEventsCLIOptions(tc.args, &bytes.Buffer{})
			if err == nil || !strings.Contains(err.Error(), "expected RFC3339 timestamp") {
				t.Fatalf("err = %v", err)
			}
		})
	}
}

func TestParseEventsCLIOptionsRejectsEmptyType(t *testing.T) {
	var stderr bytes.Buffer
	_, err := parseEventsCLIOptions([]string{"--type", "   "}, &stderr)
	if err == nil || !strings.Contains(err.Error(), "must not be empty") {
		t.Fatalf("err = %v, stderr = %q", err, stderr.String())
	}
}

func TestParseEventsCLIOptionsRejectsTrailingArguments(t *testing.T) {
	_, err := parseEventsCLIOptions([]string{"--json", "unexpected"}, &bytes.Buffer{})
	if err == nil || !strings.Contains(err.Error(), "unexpected positional argument") {
		t.Fatalf("err = %v", err)
	}
}

func TestParseEventsCLIOptionsRejectsUnknownFlag(t *testing.T) {
	_, err := parseEventsCLIOptions([]string{"--jsno"}, &bytes.Buffer{})
	if err == nil {
		t.Fatal("expected unknown flag error")
	}
}
