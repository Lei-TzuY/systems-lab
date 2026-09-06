//go:build linux

package cgroups

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func fakeCgroupFile(t *testing.T, dir, name, value string) string {
	t.Helper()
	path := filepath.Join(dir, name)
	if err := os.WriteFile(path, []byte(value), 0o600); err != nil {
		t.Fatalf("create fake cgroup file %s: %v", name, err)
	}
	return path
}

func readFakeCgroupFile(t *testing.T, path string) string {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read fake cgroup file %s: %v", path, err)
	}
	return string(data)
}

func TestConfigureV2WritesLimitsBeforeAttach(t *testing.T) {
	dir := t.TempDir()
	procs := fakeCgroupFile(t, dir, "cgroup.procs", "unattached")
	memory := fakeCgroupFile(t, dir, "memory.max", "max")
	swap := fakeCgroupFile(t, dir, "memory.swap.max", "max")
	weight := fakeCgroupFile(t, dir, "cpu.weight", "100")
	cpuMax := fakeCgroupFile(t, dir, "cpu.max", "max 100000")
	pids := fakeCgroupFile(t, dir, "pids.max", "max")

	cfg := Config{
		MemoryMax: 256 * 1024 * 1024,
		CPUWeight: 750,
		CPUs:      1.5,
		PidsMax:   64,
	}
	if err := configureV2(dir, 4321, cfg, false); err != nil {
		t.Fatalf("configureV2: %v", err)
	}

	checks := map[string]string{
		memory: "268435456",
		swap:   "0",
		weight: "750",
		cpuMax: "150000 100000",
		pids:   "64",
		procs:  "4321",
	}
	for path, want := range checks {
		if got := readFakeCgroupFile(t, path); got != want {
			t.Fatalf("%s = %q, want %q", filepath.Base(path), got, want)
		}
	}
}

func TestConfigureV2CPUQuotaFailureDoesNotAttachProcess(t *testing.T) {
	dir := t.TempDir()
	procs := fakeCgroupFile(t, dir, "cgroup.procs", "unattached")
	if err := os.Mkdir(filepath.Join(dir, "cpu.max"), 0o755); err != nil {
		t.Fatal(err)
	}

	err := configureV2(dir, 4321, Config{CPUs: 1}, false)
	if err == nil || !strings.Contains(err.Error(), "cpu.max") {
		t.Fatalf("configureV2 error=%v, want cpu.max failure", err)
	}
	if got := readFakeCgroupFile(t, procs); got != "unattached" {
		t.Fatalf("process attached despite failed CPU quota: cgroup.procs=%q", got)
	}
}

func TestConfigureV2OptionalSwapAbsenceIsAllowed(t *testing.T) {
	dir := t.TempDir()
	procs := fakeCgroupFile(t, dir, "cgroup.procs", "unattached")
	memory := fakeCgroupFile(t, dir, "memory.max", "max")

	if err := configureV2(dir, 77, Config{MemoryMax: 4096}, false); err != nil {
		t.Fatalf("missing optional memory.swap.max should be allowed: %v", err)
	}
	if got := readFakeCgroupFile(t, memory); got != "4096" {
		t.Fatalf("memory.max=%q", got)
	}
	if got := readFakeCgroupFile(t, procs); got != "77" {
		t.Fatalf("cgroup.procs=%q", got)
	}
}

func TestConfigureV2ExistingSwapWriteFailureDoesNotAttach(t *testing.T) {
	dir := t.TempDir()
	procs := fakeCgroupFile(t, dir, "cgroup.procs", "unattached")
	_ = fakeCgroupFile(t, dir, "memory.max", "max")
	if err := os.Mkdir(filepath.Join(dir, "memory.swap.max"), 0o755); err != nil {
		t.Fatal(err)
	}

	err := configureV2(dir, 88, Config{MemoryMax: 8192}, false)
	if err == nil || !strings.Contains(err.Error(), "memory.swap.max") {
		t.Fatalf("configureV2 error=%v, want existing swap knob write failure", err)
	}
	if got := readFakeCgroupFile(t, procs); got != "unattached" {
		t.Fatalf("process attached despite swap write failure: %q", got)
	}
}

func TestConfigureV2PidsFailureDoesNotAttach(t *testing.T) {
	dir := t.TempDir()
	procs := fakeCgroupFile(t, dir, "cgroup.procs", "unattached")
	if err := os.Mkdir(filepath.Join(dir, "pids.max"), 0o755); err != nil {
		t.Fatal(err)
	}

	err := configureV2(dir, 99, Config{PidsMax: 5}, false)
	if err == nil || !strings.Contains(err.Error(), "pids.max") {
		t.Fatalf("configureV2 error=%v, want pids.max failure", err)
	}
	if got := readFakeCgroupFile(t, procs); got != "unattached" {
		t.Fatalf("process attached despite pids failure: %q", got)
	}
}

func TestConfigureV2AttachFailureIsReported(t *testing.T) {
	dir := t.TempDir()
	if err := os.Mkdir(filepath.Join(dir, "cgroup.procs"), 0o755); err != nil {
		t.Fatal(err)
	}

	err := configureV2(dir, 123, Config{}, false)
	if err == nil || !strings.Contains(err.Error(), "cgroup.procs") {
		t.Fatalf("configureV2 error=%v, want attach failure", err)
	}
}
