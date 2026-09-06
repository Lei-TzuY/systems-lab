//go:build linux

package cgroups

import (
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"testing"
)

func TestAttachExistingV2WritesLiveProcessPID(t *testing.T) {
	cmd := exec.Command("sh", "-c", "read x")
	stdin, err := cmd.StdinPipe()
	if err != nil {
		t.Fatal(err)
	}
	if err := cmd.Start(); err != nil {
		t.Fatal(err)
	}
	defer func() {
		_ = stdin.Close()
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
	}()

	root := t.TempDir()
	name := "minicontainer-test"
	cgPath := filepath.Join(root, name)
	if err := os.Mkdir(cgPath, 0o755); err != nil {
		t.Fatal(err)
	}
	procs := filepath.Join(cgPath, "cgroup.procs")
	if err := os.WriteFile(procs, nil, 0o644); err != nil {
		t.Fatal(err)
	}

	if err := attachExistingV2At(root, cmd.Process.Pid, name, false); err != nil {
		t.Fatal(err)
	}
	got, err := os.ReadFile(procs)
	if err != nil {
		t.Fatal(err)
	}
	if want := strconv.Itoa(cmd.Process.Pid); string(got) != want {
		t.Fatalf("cgroup.procs = %q, want live child PID %q", got, want)
	}
}

func TestAttachExistingV1UsesOnlyExistingControllers(t *testing.T) {
	root := t.TempDir()
	name := "minicontainer-test"
	for _, controller := range []string{"memory", "pids"} {
		cgPath := filepath.Join(root, controller, name)
		if err := os.MkdirAll(cgPath, 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(cgPath, "tasks"), nil, 0o644); err != nil {
			t.Fatal(err)
		}
	}

	const pid = 4242
	if err := attachExistingV1At(root, pid, name, false); err != nil {
		t.Fatal(err)
	}
	for _, controller := range []string{"memory", "pids"} {
		data, err := os.ReadFile(filepath.Join(root, controller, name, "tasks"))
		if err != nil {
			t.Fatal(err)
		}
		if string(data) != "4242" {
			t.Fatalf("%s/tasks = %q, want 4242", controller, data)
		}
	}
	if _, err := os.Stat(filepath.Join(root, "cpu", name)); !os.IsNotExist(err) {
		t.Fatalf("unexpected cpu controller path: %v", err)
	}
}
