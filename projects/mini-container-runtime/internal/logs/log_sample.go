// Package logs provides container log processing utilities.
// This file implements systematic and rate-based log line sampling filters
// to downsample high-throughput log streams.

package logs

import (
	"fmt"
	"math/rand"
	"time"
)

// LogSampler downsamples a stream of log lines based on interval or probability.
type LogSampler struct {
	// Interval samples every Nth line (e.g. N=5 samples lines 1, 6, 11...).
	// If Interval <= 0, interval sampling is disabled.
	Interval int

	// Rate is a sampling probability between 0.0 and 1.0 (e.g. 0.25 for 25%).
	Rate float64

	isRateMode bool
	counter    int
	rng        *rand.Rand
}

// NewIntervalSampler creates a sampler that keeps every Nth log line.
func NewIntervalSampler(interval int) *LogSampler {
	if interval < 1 {
		interval = 1
	}
	return &LogSampler{
		Interval: interval,
	}
}

// NewRateSampler creates a sampler that probabilistically samples lines with rate in [0.0, 1.0].
func NewRateSampler(rate float64, seed int64) *LogSampler {
	if rate < 0 {
		rate = 0
	} else if rate > 1 {
		rate = 1
	}
	if seed == 0 {
		seed = time.Now().UnixNano()
	}
	return &LogSampler{
		Rate:       rate,
		isRateMode: true,
		rng:        rand.New(rand.NewSource(seed)),
	}
}

// SampleLine determines if the current log line should be retained.
func (s *LogSampler) SampleLine(line string) bool {
	if s.Interval > 0 {
		s.counter++
		if (s.counter-1)%s.Interval == 0 {
			return true
		}
		return false
	}

	if s.isRateMode && s.rng != nil {
		if s.Rate <= 0 {
			return false
		}
		if s.Rate >= 1 {
			return true
		}
		return s.rng.Float64() < s.Rate
	}

	return true
}

// SampleLines processes a slice of log lines and returns the sampled subset.
func (s *LogSampler) SampleLines(lines []string) []string {
	var result []string
	for _, line := range lines {
		if s.SampleLine(line) {
			result = append(result, line)
		}
	}
	return result
}

// FormatSamplingStats returns a human-readable string summarizing the sampling ratio.
func FormatSamplingStats(total, sampled int) string {
	if total == 0 {
		return "0 lines sampled (0%)"
	}
	pct := (float64(sampled) / float64(total)) * 100.0
	return fmt.Sprintf("%d/%d lines sampled (%.1f%%)", sampled, total, pct)
}
