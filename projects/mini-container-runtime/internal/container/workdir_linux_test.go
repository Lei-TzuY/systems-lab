//go:build linux

package container

import (
	"errors"
	"os"
	"strings"
	"testing"
)

func TestEnterWorkDirDefaultDoesNothing(t *testing.T) {
	for _, workDir := range []string{"", "/"} {
		mkdirCalls := 0
		chdirCalls := 0
		err := enterWorkDirWith(workDir,
			func(path string, perm os.FileMode) error { mkdirCalls++; return errors.New("must not run") },
			func(path string) error { chdirCalls++; return errors.New("must not run") },
		)
		if err != nil {
			t.Fatalf("workDir %q returned error: %v", workDir, err)
		}
		if mkdirCalls != 0 || chdirCalls != 0 {
			t.Fatalf("workDir %q invoked filesystem operations: mkdir=%d chdir=%d", workDir, mkdirCalls, chdirCalls)
		}
	}
}

func TestEnterWorkDirMkdirFailureIsFatal(t *testing.T) {
	cause := errors.New("mkdir denied")
	chdirCalls := 0
	err := enterWorkDirWith("/app/data",
		func(path string, perm os.FileMode) error {
			if path != "/app/data" || perm != 0o755 {
				t.Fatalf("unexpected mkdir args: %q %#o", path, perm)
			}
			return cause
		},
		func(path string) error { chdirCalls++; return nil },
	)
	if !errors.Is(err, cause) || !strings.Contains(err.Error(), "create workdir /app/data") {
		t.Fatalf("mkdir failure not preserved: %v", err)
	}
	if chdirCalls != 0 {
		t.Fatalf("chdir called %d times after mkdir failure", chdirCalls)
	}
}

func TestEnterWorkDirChdirFailureIsFatal(t *testing.T) {
	cause := errors.New("chdir denied")
	mkdirCalls := 0
	err := enterWorkDirWith("/app",
		func(path string, perm os.FileMode) error { mkdirCalls++; return nil },
		func(path string) error {
			if path != "/app" {
				t.Fatalf("unexpected chdir path %q", path)
			}
			return cause
		},
	)
	if mkdirCalls != 1 {
		t.Fatalf("mkdir calls=%d, want 1", mkdirCalls)
	}
	if !errors.Is(err, cause) || !strings.Contains(err.Error(), "chdir workdir /app") {
		t.Fatalf("chdir failure not preserved: %v", err)
	}
}

func TestEnterWorkDirSuccessOrdersMkdirThenChdir(t *testing.T) {
	var order []string
	err := enterWorkDirWith("/workspace",
		func(path string, perm os.FileMode) error { order = append(order, "mkdir"); return nil },
		func(path string) error { order = append(order, "chdir"); return nil },
	)
	if err != nil {
		t.Fatalf("enterWorkDirWith: %v", err)
	}
	if len(order) != 2 || order[0] != "mkdir" || order[1] != "chdir" {
		t.Fatalf("operation order=%v, want [mkdir chdir]", order)
	}
}

func TestEnterWorkDirRejectsNilOperations(t *testing.T) {
	if err := enterWorkDirWith("/app", nil, func(string) error { return nil }); err == nil {
		t.Fatal("nil mkdir unexpectedly accepted")
	}
	if err := enterWorkDirWith("/app", func(string, os.FileMode) error { return nil }, nil); err == nil {
		t.Fatal("nil chdir unexpectedly accepted")
	}
}
