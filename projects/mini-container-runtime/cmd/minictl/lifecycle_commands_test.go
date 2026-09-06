package main

import (
	"testing"
	"time"
)

func TestParseStopCommandArgsSupportsAliases(t *testing.T) {
	tests := []struct {
		name       string
		args       []string
		want       time.Duration
		wantSignal string
	}{
		{name: "default", args: []string{"ctr"}, want: 10 * time.Second, wantSignal: "SIGTERM"},
		{name: "short timeout", args: []string{"-t", "3", "ctr"}, want: 3 * time.Second, wantSignal: "SIGTERM"},
		{name: "long timeout", args: []string{"--timeout", "4", "ctr"}, want: 4 * time.Second, wantSignal: "SIGTERM"},
		{name: "long timeout equals", args: []string{"--timeout=5", "ctr"}, want: 5 * time.Second, wantSignal: "SIGTERM"},
		{name: "short signal", args: []string{"-s", "SIGINT", "ctr"}, want: 10 * time.Second, wantSignal: "SIGINT"},
		{name: "long signal", args: []string{"--signal", "SIGQUIT", "ctr"}, want: 10 * time.Second, wantSignal: "SIGQUIT"},
		{name: "numeric signal", args: []string{"--signal=10", "ctr"}, want: 10 * time.Second, wantSignal: "10"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := parseStopCommandArgs(tt.args)
			if err != nil {
				t.Fatalf("parseStopCommandArgs: %v", err)
			}
			if got.containerID != "ctr" || got.timeout != tt.want || got.signal != tt.wantSignal {
				t.Fatalf("got id=%q timeout=%v signal=%q, want ctr/%v/%q", got.containerID, got.timeout, got.signal, tt.want, tt.wantSignal)
			}
		})
	}
}

func TestParseStopCommandArgsRejectsUnsafeInput(t *testing.T) {
	for _, args := range [][]string{
		{"--timeout", "-1", "ctr"},
		{"--signal", "NOTASIGNAL", "ctr"},
		{},
		{"ctr", "extra"},
	} {
		if _, err := parseStopCommandArgs(args); err == nil {
			t.Fatalf("parseStopCommandArgs(%v) succeeded, want error", args)
		}
	}
}

func TestParseKillCommandArgsSupportsAliases(t *testing.T) {
	tests := []struct {
		name string
		args []string
		want string
	}{
		{name: "default", args: []string{"ctr"}, want: "SIGKILL"},
		{name: "short", args: []string{"-s", "SIGTERM", "ctr"}, want: "SIGTERM"},
		{name: "long", args: []string{"--signal", "SIGINT", "ctr"}, want: "SIGINT"},
		{name: "numeric", args: []string{"--signal=15", "ctr"}, want: "15"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := parseKillCommandArgs(tt.args)
			if err != nil {
				t.Fatalf("parseKillCommandArgs: %v", err)
			}
			if got.containerID != "ctr" || got.signal != tt.want {
				t.Fatalf("got id=%q signal=%q, want ctr/%q", got.containerID, got.signal, tt.want)
			}
		})
	}
}

func TestParseKillCommandArgsRejectsInvalidSignalAndArity(t *testing.T) {
	for _, args := range [][]string{
		{"--signal", "NOTASIGNAL", "ctr"},
		{},
		{"ctr", "extra"},
	} {
		if _, err := parseKillCommandArgs(args); err == nil {
			t.Fatalf("parseKillCommandArgs(%v) succeeded, want error", args)
		}
	}
}

func TestShortContainerID(t *testing.T) {
	if got := shortContainerID("1234567890abcdef"); got != "12345678" {
		t.Fatalf("shortContainerID long=%q", got)
	}
	if got := shortContainerID("abc"); got != "abc" {
		t.Fatalf("shortContainerID short=%q", got)
	}
}
