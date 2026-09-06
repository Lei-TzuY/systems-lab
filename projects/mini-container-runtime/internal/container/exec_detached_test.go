package container

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestExecDetached(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	c := &state.Container{
		ID:        "ctr-det-1",
		Status:    state.StatusRunning,
		PID:       1234,
		CreatedAt: time.Now(),
	}
	_ = st.Save(c)

	pid, err := ExecDetached(st, c.ID, []string{"echo", "hello"})
	if err != nil {
		if runtime.GOOS == "linux" && os.IsNotExist(err) {
			t.Skipf("nsenter unavailable: %v", err)
		}
		t.Fatalf("ExecDetached error: %v", err)
	}
	if pid == 0 {
		t.Fatalf("Returned pid is 0")
	}
}

func TestResolveTrustedExecutableIgnoresPATH(t *testing.T) {
	attackerDir := t.TempDir()
	trustedDir := t.TempDir()

	attacker := filepath.Join(attackerDir, "nsenter")
	if err := os.WriteFile(attacker, []byte("#!/bin/sh\nexit 0\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	trusted := filepath.Join(trustedDir, "nsenter")
	if err := os.WriteFile(trusted, []byte("#!/bin/sh\nexit 0\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", attackerDir)

	got, err := resolveTrustedExecutable("nsenter", []string{trustedDir})
	if err != nil {
		t.Fatalf("resolve trusted executable: %v", err)
	}
	if got != trusted {
		t.Fatalf("resolved %q, want trusted %q", got, trusted)
	}
}

func TestResolveTrustedExecutableRejectsSymlink(t *testing.T) {
	dir := t.TempDir()
	victim := filepath.Join(t.TempDir(), "victim")
	if err := os.WriteFile(victim, []byte("#!/bin/sh\nexit 0\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	candidate := filepath.Join(dir, "nsenter")
	if err := os.Symlink(victim, candidate); err != nil {
		t.Skipf("symlink unsupported: %v", err)
	}

	if got, err := resolveTrustedExecutable("nsenter", []string{dir}); err == nil {
		t.Fatalf("resolved unsafe symlink %q", got)
	}
}
