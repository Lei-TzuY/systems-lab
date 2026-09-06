package bench

import (
	"testing"

	"minicontainer/internal/state"
)

func TestRunBenchmark(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	res, err := RunBenchmark(st, 10)
	if err != nil {
		t.Fatalf("RunBenchmark error: %v", err)
	}

	if res.Iterations != 10 {
		t.Fatalf("BenchResult Iterations = %d, want 10", res.Iterations)
	}
}
