// Package logs provides container log processing utilities.
// This file implements log stream sampling algorithms (deterministic 1-in-N,
// rate-based sampling, and fixed-capacity reservoir sampling).

package logs

import (
	"math/rand"
)

// SampleEveryN returns every n-th log line (1-in-N deterministic sampling).
// If n <= 1, all lines are returned.
func SampleEveryN(lines []string, n int) []string {
	if n <= 1 || len(lines) == 0 {
		return lines
	}

	var sampled []string
	for i := 0; i < len(lines); i += n {
		sampled = append(sampled, lines[i])
	}
	return sampled
}

// SampleFraction returns a subset of lines sampled at rate (0.0 < rate <= 1.0).
// seed provides deterministic reproducibility for testing or distributed nodes.
func SampleFraction(lines []string, rate float64, seed int64) []string {
	if rate >= 1.0 || len(lines) == 0 {
		return lines
	}
	if rate <= 0.0 {
		return nil
	}

	r := rand.New(rand.NewSource(seed))
	var sampled []string
	for _, line := range lines {
		if r.Float64() < rate {
			sampled = append(sampled, line)
		}
	}
	return sampled
}

// ReservoirSample selects exactly k lines uniformly at random from a stream
// using Algorithm R (Reservoir Sampling). If len(lines) <= k, all lines are returned.
func ReservoirSample(lines []string, k int, seed int64) []string {
	n := len(lines)
	if k <= 0 || n == 0 {
		return nil
	}
	if n <= k {
		out := make([]string, n)
		copy(out, lines)
		return out
	}

	reservoir := make([]string, k)
	copy(reservoir, lines[:k])

	r := rand.New(rand.NewSource(seed))
	for i := k; i < n; i++ {
		j := r.Intn(i + 1)
		if j < k {
			reservoir[j] = lines[i]
		}
	}

	return reservoir
}
