package cgroups

import (
	"math"
	"strings"
	"testing"
)

func TestValidateCgroupName(t *testing.T) {
	valid := []string{
		"minicontainer-1234",
		"ctr_1.2-test",
		"A",
	}
	for _, name := range valid {
		if err := validateCgroupName(name); err != nil {
			t.Fatalf("valid cgroup name %q rejected: %v", name, err)
		}
	}

	invalid := []string{
		"",
		".",
		"..",
		"../escape",
		"nested/name",
		`nested\name`,
		"has space",
		"line\nbreak",
		"unicode-容器",
		strings.Repeat("a", maxCgroupNameLen+1),
	}
	for _, name := range invalid {
		if err := validateCgroupName(name); err == nil {
			t.Fatalf("invalid cgroup name %q accepted", name)
		}
	}
}

func TestValidateResourceValues(t *testing.T) {
	valid := []struct {
		memory int64
		weight int64
		cpus   float64
		pids   int64
	}{
		{0, 0, 0, 0},
		{1, 1, 0.1, 1},
		{1 << 30, 10000, 64, 32768},
	}
	for _, tc := range valid {
		if err := validateResourceValues(tc.memory, tc.weight, tc.cpus, tc.pids); err != nil {
			t.Fatalf("valid resource values rejected: %+v: %v", tc, err)
		}
	}

	invalid := []struct {
		name   string
		memory int64
		weight int64
		cpus   float64
		pids   int64
	}{
		{"negative memory", -1, 0, 0, 0},
		{"negative weight", 0, -1, 0, 0},
		{"weight too high", 0, 10001, 0, 0},
		{"negative CPUs", 0, 0, -0.1, 0},
		{"NaN CPUs", 0, 0, math.NaN(), 0},
		{"infinite CPUs", 0, 0, math.Inf(1), 0},
		{"overflowing CPUs", 0, 0, math.MaxFloat64, 0},
		{"negative pids", 0, 0, 0, -1},
	}
	for _, tc := range invalid {
		t.Run(tc.name, func(t *testing.T) {
			if err := validateResourceValues(tc.memory, tc.weight, tc.cpus, tc.pids); err == nil {
				t.Fatalf("invalid resource values accepted: %+v", tc)
			}
		})
	}
}
