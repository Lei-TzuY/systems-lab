//go:build linux

package container

import (
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
)

func TestStartContainerProcessPublishesPinnedRootFSFDToBootstrap(t *testing.T) {
	rootfs := filepath.Join(t.TempDir(), "rootfs")
	if err := os.Mkdir(rootfs, 0o755); err != nil {
		t.Fatal(err)
	}
	admitted, err := os.Stat(rootfs)
	if err != nil {
		t.Fatal(err)
	}

	command := []string{"payload"}
	cmd := managedShellCommand(rootfs, command, "exit 0")
	originalExtraFiles := len(cmd.ExtraFiles)
	if err := startContainerProcess(Config{RootFS: rootfs, RootFSIdentity: admitted, Command: command}, cmd); err != nil {
		t.Fatalf("start managed process: %v", err)
	}
	defer cmd.Wait()

	want := pinnedRootFSEnvKey + "=" + strconv.Itoa(3+originalExtraFiles)
	found := false
	for _, entry := range cmd.Env {
		if entry == want {
			found = true
			break
		}
	}
	if !found {
		t.Fatalf("managed child environment missing %q; env=%q", want, strings.Join(cmd.Env, " "))
	}
}

func TestStartContainerProcessLeavesUnmanagedEnvironmentUntouched(t *testing.T) {
	rootfs := t.TempDir()
	command := []string{"payload"}
	cmd := managedShellCommand(rootfs, command, "exit 0")
	cmd.Env = []string{"EXISTING=value"}

	if err := startContainerProcess(Config{RootFS: rootfs, Command: command}, cmd); err != nil {
		t.Fatalf("start unmanaged process: %v", err)
	}
	defer cmd.Wait()

	if len(cmd.Env) != 1 || cmd.Env[0] != "EXISTING=value" {
		t.Fatalf("unmanaged environment mutated: %q", cmd.Env)
	}
}
