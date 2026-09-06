//go:build linux

package cgroups

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestReadStatsAtPathSuccess(t *testing.T) {
	dir := t.TempDir()
	fixtures := map[string]string{
		"memory.current":  "67108864\n",
		"memory.max":      "134217728\n",
		"pids.current":    "7\n",
		"cpu.stat":        "usage_usec 1250000\nuser_usec 750000\nsystem_usec 500000\n",
		"cpu.pressure":    "some avg10=1.25 avg60=2.50 avg300=3.75 total=500000\n",
		"memory.pressure": "some avg10=4.00 avg60=5.00 avg300=6.00 total=1000000\nfull avg10=0.25 avg60=0.50 avg300=0.75 total=250000\n",
		"io.pressure":     "some avg10=0.10 avg60=0.20 avg300=0.30 total=10000\nfull avg10=0.01 avg60=0.02 avg300=0.03 total=1000\n",
	}
	for name, content := range fixtures {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(content), 0o644); err != nil {
			t.Fatalf("write %s: %v", name, err)
		}
	}

	stats, err := readStatsAtPath(dir)
	if err != nil {
		t.Fatalf("readStatsAtPath error: %v", err)
	}
	if stats.MemoryUsage != 67108864 || stats.MemoryLimit != 134217728 || stats.PidsCurrent != 7 || stats.CPUUsageUsec != 1250000 {
		t.Fatalf("unexpected counters: %+v", stats)
	}
	if stats.CPUPressure == nil || stats.CPUPressure.Some.Total != 500000 {
		t.Fatalf("unexpected CPU pressure: %+v", stats.CPUPressure)
	}
	if stats.MemoryPressure == nil || stats.MemoryPressure.Full == nil || stats.MemoryPressure.Full.Total != 250000 {
		t.Fatalf("unexpected memory pressure: %+v", stats.MemoryPressure)
	}
	if stats.IOPressure == nil || stats.IOPressure.Full == nil || stats.IOPressure.Full.Total != 1000 {
		t.Fatalf("unexpected IO pressure: %+v", stats.IOPressure)
	}
}

func TestReadStatsAtPathAllowsUnlimitedMemoryAndMissingOptionalFiles(t *testing.T) {
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "memory.max"), []byte("max\n"), 0o644); err != nil {
		t.Fatal(err)
	}

	stats, err := readStatsAtPath(dir)
	if err != nil {
		t.Fatalf("readStatsAtPath error: %v", err)
	}
	if stats.MemoryLimit != 0 {
		t.Fatalf("expected unlimited memory limit to be 0, got %d", stats.MemoryLimit)
	}
	if stats.CPUPressure != nil || stats.MemoryPressure != nil || stats.IOPressure != nil {
		t.Fatalf("missing PSI files should remain nil: %+v", stats)
	}
}

func TestReadStatsAtPathRejectsMalformedCounters(t *testing.T) {
	tests := []struct {
		name     string
		file     string
		contents string
		want     string
	}{
		{name: "memory current", file: "memory.current", contents: "not-a-number\n", want: "memory.current"},
		{name: "negative memory current", file: "memory.current", contents: "-1\n", want: "must not be negative"},
		{name: "memory max", file: "memory.max", contents: "invalid\n", want: "memory.max"},
		{name: "negative memory max", file: "memory.max", contents: "-10\n", want: "must not be negative"},
		{name: "pids current", file: "pids.current", contents: "oops\n", want: "pids.current"},
		{name: "negative pids", file: "pids.current", contents: "-2\n", want: "must not be negative"},
		{name: "cpu usage", file: "cpu.stat", contents: "usage_usec nope\n", want: "usage_usec"},
		{name: "cpu missing usage", file: "cpu.stat", contents: "user_usec 1\nsystem_usec 2\n", want: "missing usage_usec"},
		{name: "cpu malformed usage line", file: "cpu.stat", contents: "usage_usec 1 extra\n", want: "malformed"},
		{name: "malformed PSI", file: "memory.pressure", contents: "some avg10=oops avg60=0 avg300=0 total=0\n", want: "memory.pressure"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			dir := t.TempDir()
			if err := os.WriteFile(filepath.Join(dir, tt.file), []byte(tt.contents), 0o644); err != nil {
				t.Fatal(err)
			}
			_, err := readStatsAtPath(dir)
			if err == nil {
				t.Fatal("expected error")
			}
			if !strings.Contains(err.Error(), tt.want) {
				t.Fatalf("error %q does not contain %q", err, tt.want)
			}
		})
	}
}

func TestReadStatsRejectsPathLikeNames(t *testing.T) {
	for _, name := range []string{"", ".", "..", "../escape", "/absolute", "nested/name"} {
		t.Run(strings.ReplaceAll(name, "/", "_"), func(t *testing.T) {
			if _, err := ReadStats(name); err == nil {
				t.Fatalf("expected invalid cgroup name %q to fail", name)
			}
		})
	}
}

func TestReadStatsAtPathRejectsNonDirectory(t *testing.T) {
	path := filepath.Join(t.TempDir(), "not-a-directory")
	if err := os.WriteFile(path, []byte("x"), 0o644); err != nil {
		t.Fatal(err)
	}
	if _, err := readStatsAtPath(path); err == nil || !strings.Contains(err.Error(), "not a directory") {
		t.Fatalf("expected not-a-directory error, got %v", err)
	}
}
