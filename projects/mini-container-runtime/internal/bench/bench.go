package bench

import (
	"fmt"
	"time"

	"minicontainer/internal/state"
)

type BenchResult struct {
	StartupLatencyMs float64 `json:"startup_latency_ms"`
	StateReadMs      float64 `json:"state_read_ms"`
	StateWriteMs     float64 `json:"state_write_ms"`
	Iterations       int     `json:"iterations"`
}

// RunBenchmark measures container engine state store operations latency.
func RunBenchmark(st *state.Store, iterations int) (*BenchResult, error) {
	if st == nil {
		return nil, fmt.Errorf("state store is nil")
	}
	if iterations <= 0 {
		iterations = 50
	}

	res := &BenchResult{Iterations: iterations}

	// Measure write latency
	startWrite := time.Now()
	for i := 0; i < iterations; i++ {
		c := &state.Container{
			ID:        fmt.Sprintf("bench-ctr-%d", i),
			Status:    state.StatusStopped,
			CreatedAt: time.Now(),
		}
		_ = st.Save(c)
	}
	writeElapsed := time.Since(startWrite)
	res.StateWriteMs = float64(writeElapsed.Milliseconds()) / float64(iterations)

	// Measure read latency
	startRead := time.Now()
	for i := 0; i < iterations; i++ {
		_, _ = st.List()
	}
	readElapsed := time.Since(startRead)
	res.StateReadMs = float64(readElapsed.Milliseconds()) / float64(iterations)

	// Cleanup test bench containers
	for i := 0; i < iterations; i++ {
		_ = st.Delete(fmt.Sprintf("bench-ctr-%d", i))
	}

	res.StartupLatencyMs = res.StateWriteMs + res.StateReadMs
	return res, nil
}
