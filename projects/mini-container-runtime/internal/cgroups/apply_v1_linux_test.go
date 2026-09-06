//go:build linux

package cgroups

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func prepareFakeV1Hierarchy(t *testing.T, root, name string, plans []v1ControllerPlan, includeOptional bool) {
	t.Helper()
	for _, plan := range plans {
		controllerRoot := filepath.Join(root, plan.controller)
		if err := os.MkdirAll(controllerRoot, 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(controllerRoot, "tasks"), []byte("parent"), 0o600); err != nil {
			t.Fatal(err)
		}
		cgPath := filepath.Join(controllerRoot, name)
		if err := os.Mkdir(cgPath, 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(cgPath, "tasks"), []byte("unattached"), 0o600); err != nil {
			t.Fatal(err)
		}
		for _, write := range plan.writes {
			if write.optional && !includeOptional {
				continue
			}
			if err := os.WriteFile(filepath.Join(cgPath, write.file), []byte("initial"), 0o600); err != nil {
				t.Fatal(err)
			}
		}
	}
}

func readV1TestFile(t *testing.T, path string) string {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}

func TestBuildV1PlansCombinesCPUControls(t *testing.T) {
	plans := buildV1Plans(Config{MemoryMax: 1024, CPUWeight: 200, CPUs: 1.5, PidsMax: 8})
	if len(plans) != 3 {
		t.Fatalf("plans=%d, want memory/cpu/pids", len(plans))
	}
	if plans[0].controller != "memory" || plans[1].controller != "cpu" || plans[2].controller != "pids" {
		t.Fatalf("unexpected plan order: %+v", plans)
	}
	if len(plans[1].writes) != 3 {
		t.Fatalf("cpu writes=%d, want shares+period+quota", len(plans[1].writes))
	}
}

func TestApplyV1PreparedConfiguresAllLimitsBeforeAnyAttach(t *testing.T) {
	root := t.TempDir()
	const name = "ctr-v1-order"
	cfg := Config{MemoryMax: 4096, CPUWeight: 250, CPUs: 1, PidsMax: 7}
	plans := buildV1Plans(cfg)
	prepareFakeV1Hierarchy(t, root, name, plans, true)

	// Make a late CPU configuration write fail. Memory configuration is earlier,
	// but no controller may receive the PID until every plan is configured.
	quota := filepath.Join(root, "cpu", name, "cpu.cfs_quota_us")
	if err := os.Remove(quota); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(quota, 0o755); err != nil {
		t.Fatal(err)
	}

	err := applyV1Prepared(root, name, 4321, plans, false)
	if err == nil || !strings.Contains(err.Error(), "cpu.cfs_quota_us") {
		t.Fatalf("error=%v, want CPU quota write failure", err)
	}
	for _, controller := range []string{"memory", "cpu", "pids"} {
		got := readV1TestFile(t, filepath.Join(root, controller, name, "tasks"))
		if got != "unattached" {
			t.Fatalf("%s/tasks=%q after configuration failure, want unattached", controller, got)
		}
	}
}

func TestApplyV1PreparedWritesExactValuesThenAttaches(t *testing.T) {
	root := t.TempDir()
	const name = "ctr-v1-success"
	cfg := Config{MemoryMax: 8192, CPUWeight: 300, CPUs: 1.5, PidsMax: 9}
	plans := buildV1Plans(cfg)
	prepareFakeV1Hierarchy(t, root, name, plans, true)

	if err := applyV1Prepared(root, name, 77, plans, false); err != nil {
		t.Fatalf("applyV1Prepared: %v", err)
	}
	checks := map[string]string{
		filepath.Join(root, "memory", name, "memory.limit_in_bytes"):       "8192",
		filepath.Join(root, "memory", name, "memory.memsw.limit_in_bytes"): "8192",
		filepath.Join(root, "cpu", name, "cpu.shares"):                    "300",
		filepath.Join(root, "cpu", name, "cpu.cfs_period_us"):             "100000",
		filepath.Join(root, "cpu", name, "cpu.cfs_quota_us"):              "150000",
		filepath.Join(root, "pids", name, "pids.max"):                      "9",
	}
	for path, want := range checks {
		if got := readV1TestFile(t, path); got != want {
			t.Fatalf("%s=%q, want %q", path, got, want)
		}
	}
	for _, controller := range []string{"memory", "cpu", "pids"} {
		if got := readV1TestFile(t, filepath.Join(root, controller, name, "tasks")); got != "77" {
			t.Fatalf("%s/tasks=%q, want 77", controller, got)
		}
	}
}

func TestApplyV1PreparedAllowsMissingOptionalMemsw(t *testing.T) {
	root := t.TempDir()
	const name = "ctr-v1-no-memsw"
	plans := buildV1Plans(Config{MemoryMax: 16384})
	prepareFakeV1Hierarchy(t, root, name, plans, false)

	if err := applyV1Prepared(root, name, 88, plans, false); err != nil {
		t.Fatalf("missing optional memsw should be accepted: %v", err)
	}
	if got := readV1TestFile(t, filepath.Join(root, "memory", name, "tasks")); got != "88" {
		t.Fatalf("memory/tasks=%q, want 88", got)
	}
}

func TestApplyV1PreparedRollsBackEarlierAttach(t *testing.T) {
	root := t.TempDir()
	const name = "ctr-v1-rollback"
	plans := buildV1Plans(Config{MemoryMax: 4096, CPUWeight: 200})
	prepareFakeV1Hierarchy(t, root, name, plans, false)

	cpuTasks := filepath.Join(root, "cpu", name, "tasks")
	if err := os.Remove(cpuTasks); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(cpuTasks, 0o755); err != nil {
		t.Fatal(err)
	}

	err := applyV1Prepared(root, name, 99, plans, false)
	if err == nil || !strings.Contains(err.Error(), "attach PID 99") {
		t.Fatalf("error=%v, want attach failure", err)
	}
	// On a real v1 hierarchy this parent tasks write moves the PID back out of
	// the child cgroup. The fake hierarchy records the rollback write directly.
	if got := readV1TestFile(t, filepath.Join(root, "memory", "tasks")); got != "99" {
		t.Fatalf("memory parent tasks=%q, want rollback PID 99", got)
	}
}

func TestApplyV1PreparedPreservesAttachAndRollbackErrors(t *testing.T) {
	root := t.TempDir()
	const name = "ctr-v1-rollback-errors"
	plans := buildV1Plans(Config{MemoryMax: 4096, CPUWeight: 200})
	prepareFakeV1Hierarchy(t, root, name, plans, false)

	cpuTasks := filepath.Join(root, "cpu", name, "tasks")
	if err := os.Remove(cpuTasks); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(cpuTasks, 0o755); err != nil {
		t.Fatal(err)
	}
	memoryParentTasks := filepath.Join(root, "memory", "tasks")
	if err := os.Remove(memoryParentTasks); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(memoryParentTasks, 0o755); err != nil {
		t.Fatal(err)
	}

	err := applyV1Prepared(root, name, 101, plans, false)
	if err == nil {
		t.Fatal("expected attach+rollback failure")
	}
	if !strings.Contains(err.Error(), "attach PID 101") || !strings.Contains(err.Error(), "rollback PID 101") {
		t.Fatalf("joined failure lost context: %v", err)
	}
}

func TestApplyV1AtRejectsExistingCgroup(t *testing.T) {
	root := t.TempDir()
	const name = "ctr-v1-stale"
	controllerRoot := filepath.Join(root, "pids")
	if err := os.MkdirAll(filepath.Join(controllerRoot, name), 0o755); err != nil {
		t.Fatal(err)
	}

	err := applyV1At(root, 123, Config{Name: name, PidsMax: 5}, false)
	if err == nil || !errors.Is(err, os.ErrExist) && !strings.Contains(err.Error(), "already exists") {
		t.Fatalf("existing cgroup error=%v", err)
	}
}
