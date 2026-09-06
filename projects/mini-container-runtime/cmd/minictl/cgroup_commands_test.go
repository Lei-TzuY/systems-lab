package main

import "testing"

func TestParseUpdateCommandArgs(t *testing.T) {
	got, err := parseUpdateCommandArgs([]string{
		"--memory", "64m",
		"--cpus", "1.5",
		"--cpu-weight", "100",
		"--pids-limit", "20",
		"ctr",
	})
	if err != nil {
		t.Fatalf("parseUpdateCommandArgs: %v", err)
	}
	if got.containerID != "ctr" {
		t.Fatalf("containerID = %q, want ctr", got.containerID)
	}
	if got.config.MemoryMax != 64*1024*1024 {
		t.Fatalf("MemoryMax = %d, want %d", got.config.MemoryMax, 64*1024*1024)
	}
	if got.config.CPUs != 1.5 || got.config.CPUWeight != 100 || got.config.PidsMax != 20 {
		t.Fatalf("config = %+v", got.config)
	}
}

func TestParseUpdateCommandArgsRejectsInvalidInput(t *testing.T) {
	for _, args := range [][]string{
		nil,
		{"ctr", "extra"},
		{"--memory", "bad", "ctr"},
	} {
		if _, err := parseUpdateCommandArgs(args); err == nil {
			t.Fatalf("parseUpdateCommandArgs(%v) succeeded, want error", args)
		}
	}
}
