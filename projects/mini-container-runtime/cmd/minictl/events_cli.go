package main

import (
	"flag"
	"fmt"
	"io"
	"strings"
	"time"

	"minicontainer/internal/events"
)

type eventTypeFlag []events.EventType

func (values *eventTypeFlag) String() string {
	if values == nil || len(*values) == 0 {
		return ""
	}
	parts := make([]string, 0, len(*values))
	for _, value := range *values {
		parts = append(parts, string(value))
	}
	return strings.Join(parts, ",")
}

func (values *eventTypeFlag) Set(value string) error {
	value = strings.TrimSpace(value)
	if value == "" {
		return fmt.Errorf("event type must not be empty")
	}
	*values = append(*values, events.EventType(value))
	return nil
}

func parseEventTime(value, name string) (time.Time, error) {
	value = strings.TrimSpace(value)
	if value == "" {
		return time.Time{}, fmt.Errorf("%s must not be empty", name)
	}
	parsed, err := time.Parse(time.RFC3339Nano, value)
	if err != nil {
		return time.Time{}, fmt.Errorf("invalid %s %q: expected RFC3339 timestamp: %w", name, value, err)
	}
	return parsed, nil
}

func parseEventsCLIOptions(args []string, stderr io.Writer) (events.StreamOptions, error) {
	fs := flag.NewFlagSet("events", flag.ContinueOnError)
	fs.SetOutput(stderr)

	var opts events.StreamOptions
	var types eventTypeFlag
	var sinceRaw, untilRaw string
	fs.BoolVar(&opts.Follow, "f", false, "follow real-time event log")
	fs.BoolVar(&opts.Follow, "follow", false, "follow real-time event log")
	fs.BoolVar(&opts.JSON, "json", false, "emit one JSON event per line")
	fs.StringVar(&opts.ContainerPrefix, "container", "", "filter by container ID prefix")
	fs.Var(&types, "type", "filter by lifecycle event type (repeatable)")
	fs.StringVar(&sinceRaw, "since", "", "include events at or after RFC3339 timestamp")
	fs.StringVar(&untilRaw, "until", "", "include events at or before RFC3339 timestamp")

	if err := fs.Parse(args); err != nil {
		return events.StreamOptions{}, err
	}
	if fs.NArg() != 0 {
		return events.StreamOptions{}, fmt.Errorf("unexpected positional argument %q", fs.Arg(0))
	}
	var err error
	if sinceRaw != "" {
		opts.Since, err = parseEventTime(sinceRaw, "--since")
		if err != nil {
			return events.StreamOptions{}, err
		}
	}
	if untilRaw != "" {
		opts.Until, err = parseEventTime(untilRaw, "--until")
		if err != nil {
			return events.StreamOptions{}, err
		}
	}
	opts.Types = append([]events.EventType(nil), types...)
	return opts, nil
}
