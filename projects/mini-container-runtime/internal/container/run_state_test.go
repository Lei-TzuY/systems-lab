//go:build linux

package container

import (
	"errors"
	"os"
	"os/exec"
	"testing"
)

func TestExitCodeFromWaitError(t *testing.T) {
	if got := exitCodeFromWaitError(nil); got != 0 {
		t.Fatalf("nil error exit code = %d, want 0", got)
	}
	if got := exitCodeFromWaitError(errors.New("runtime failure")); got != 1 {
		t.Fatalf("generic error exit code = %d, want 1", got)
	}
}

func TestRuntimeStateErrorIsDiscoverableThroughJoin(t *testing.T) {
	stateErr := &runtimeStateError{err: errors.New("state failed")}
	joined := errors.Join(errors.New("payload failed"), stateErr)
	var got *runtimeStateError
	if !errors.As(joined, &got) || got != stateErr {
		t.Fatalf("runtime state error not discoverable through errors.Join: %v", joined)
	}
}

func TestExitCodeFromRealProcess(t *testing.T) {
	cmd := exec.Command("sh", "-c", "exit 23")
	err := cmd.Run()
	if err == nil {
		t.Fatal("expected non-zero exit")
	}
	if got := exitCodeFromWaitError(err); got != 23 {
		t.Fatalf("exit code = %d, want 23", got)
	}
}

func TestRunClosesManagedLifecycleStoreOnPreSpawnFailure(t *testing.T) {
	before := countOpenFDs(t)
	cfg := Config{
		ContainerID: "store-close-test",
		StateDir:    t.TempDir(),
		PortMappings: []PortMapping{
			{HostPort: 18080, ContainerPort: 8080},
		},
	}

	const attempts = 32
	for i := 0; i < attempts; i++ {
		err := Run(cfg)
		if err == nil {
			t.Fatal("Run unexpectedly accepted published ports without bridge networking")
		}
		var setupErr *runtimeSetupError
		if !errors.As(err, &setupErr) {
			t.Fatalf("Run error = %T %v, want runtimeSetupError", err, err)
		}
	}

	after := countOpenFDs(t)
	if after > before {
		t.Fatalf("managed Run leaked file descriptors across pre-spawn failures: before=%d after=%d", before, after)
	}
}

func countOpenFDs(t *testing.T) int {
	t.Helper()
	entries, err := os.ReadDir("/proc/self/fd")
	if err != nil {
		t.Fatalf("read /proc/self/fd: %v", err)
	}
	return len(entries)
}
