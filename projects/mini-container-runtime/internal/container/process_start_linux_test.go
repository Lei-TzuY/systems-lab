//go:build linux

package container

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func managedShellCommand(rootfs string, command []string, script string) *exec.Cmd {
	cmdArgs := []string{"-c", script, "sh", rootfs}
	cmdArgs = append(cmdArgs, command...)
	return exec.Command("/bin/sh", cmdArgs...)
}

func TestStartContainerProcessRejectsRootFSReplacementBeforeSpawn(t *testing.T) {
	parent := t.TempDir()
	rootfs := filepath.Join(parent, "rootfs")
	if err := os.Mkdir(rootfs, 0o755); err != nil {
		t.Fatal(err)
	}
	admitted, err := os.Stat(rootfs)
	if err != nil {
		t.Fatal(err)
	}

	original := filepath.Join(parent, "original")
	if err := os.Rename(rootfs, original); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(rootfs, 0o755); err != nil {
		t.Fatal(err)
	}

	command := []string{"payload"}
	cmd := managedShellCommand(rootfs, command, "exit 0")
	err = startContainerProcess(Config{RootFS: rootfs, RootFSIdentity: admitted, Command: command}, cmd)
	if err == nil {
		t.Fatal("expected spawn-boundary rootfs identity rejection")
	}
	if !strings.Contains(err.Error(), "filesystem identity changed before runtime attempt") {
		t.Fatalf("unexpected error: %v", err)
	}
	if cmd.Process != nil {
		t.Fatalf("process was created despite rootfs identity drift: pid=%d", cmd.Process.Pid)
	}
}

func TestStartContainerProcessAllowsStableAdmittedRootFS(t *testing.T) {
	rootfs := t.TempDir()
	admitted, err := os.Stat(rootfs)
	if err != nil {
		t.Fatal(err)
	}

	command := []string{"payload"}
	cmd := managedShellCommand(rootfs, command, "test -d \"$1\"")
	if err := startContainerProcess(Config{RootFS: rootfs, RootFSIdentity: admitted, Command: command}, cmd); err != nil {
		t.Fatalf("start stable rootfs process: %v", err)
	}
	if err := cmd.Wait(); err != nil {
		t.Fatalf("wait stable rootfs process: %v", err)
	}
	rootArgIndex := len(cmd.Args) - len(command) - 1
	if got := cmd.Args[rootArgIndex]; !strings.HasPrefix(got, "/proc/self/fd/") {
		t.Fatalf("managed child rootfs was not fd-pinned: %q", got)
	}
}

func TestStartContainerProcessPinnedRootFSSurvivesPathReplacement(t *testing.T) {
	parent := t.TempDir()
	rootfs := filepath.Join(parent, "rootfs")
	if err := os.Mkdir(rootfs, 0o755); err != nil {
		t.Fatal(err)
	}
	admitted, err := os.Stat(rootfs)
	if err != nil {
		t.Fatal(err)
	}

	gate := filepath.Join(parent, "continue")
	command := []string{"payload"}
	script := fmt.Sprintf("while [ ! -e %q ]; do sleep 0.01; done; touch \"$1/pinned-marker\"", gate)
	cmd := managedShellCommand(rootfs, command, script)
	if err := startContainerProcess(Config{RootFS: rootfs, RootFSIdentity: admitted, Command: command}, cmd); err != nil {
		t.Fatalf("start fd-pinned rootfs process: %v", err)
	}

	original := filepath.Join(parent, "original")
	if err := os.Rename(rootfs, original); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(rootfs, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(gate, []byte("go"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := cmd.Wait(); err != nil {
		t.Fatalf("wait fd-pinned rootfs process: %v", err)
	}

	if _, err := os.Stat(filepath.Join(original, "pinned-marker")); err != nil {
		t.Fatalf("child did not retain admitted rootfs object: %v", err)
	}
	if _, err := os.Stat(filepath.Join(rootfs, "pinned-marker")); !os.IsNotExist(err) {
		t.Fatalf("child followed replacement rootfs pathname; stat error=%v", err)
	}
}

func TestStartContainerProcessRejectsMalformedManagedCommandBeforeSpawn(t *testing.T) {
	rootfs := t.TempDir()
	admitted, err := os.Stat(rootfs)
	if err != nil {
		t.Fatal(err)
	}

	cmd := exec.Command("/bin/sh", "-c", "exit 0")
	err = startContainerProcess(Config{RootFS: rootfs, RootFSIdentity: admitted, Command: []string{"payload"}}, cmd)
	if err == nil {
		t.Fatal("expected malformed managed child argv rejection")
	}
	if !strings.Contains(err.Error(), "could not locate child rootfs argument") {
		t.Fatalf("unexpected error: %v", err)
	}
	if cmd.Process != nil {
		t.Fatalf("process was created despite malformed managed argv: pid=%d", cmd.Process.Pid)
	}
}

func TestStartContainerProcessPreservesUnmanagedRunCompatibility(t *testing.T) {
	cmd := exec.Command("/bin/sh", "-c", "exit 0")
	if err := startContainerProcess(Config{}, cmd); err != nil {
		t.Fatalf("start unmanaged process: %v", err)
	}
	if err := cmd.Wait(); err != nil {
		t.Fatalf("wait unmanaged process: %v", err)
	}
}
