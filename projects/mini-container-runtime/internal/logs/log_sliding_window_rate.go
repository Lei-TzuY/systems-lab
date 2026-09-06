// Package logs provides container log processing utilities.
// This file implements a sliding-window rate limiter and surge detector
// to detect and throttle log flooding (e.g., crash loops, debug spam) in container streams.

package logs

import (
	"fmt"
	"time"
)

// SlidingWindowRateLimiter tracks log volume over a sliding time window.
type SlidingWindowRateLimiter struct {
	MaxLinesPerWindow int
	WindowDuration    time.Duration
	timestamps        []time.Time
}

// NewSlidingWindowRateLimiter creates a rate limiter with capacity and window duration.
func NewSlidingWindowRateLimiter(maxLines int, window time.Duration) *SlidingWindowRateLimiter {
	if maxLines <= 0 {
		maxLines = 100
	}
	if window <= 0 {
		window = time.Second
	}
	return &SlidingWindowRateLimiter{
		MaxLinesPerWindow: maxLines,
		WindowDuration:    window,
		timestamps:        make([]time.Time, 0, maxLines*2),
	}
}

// Allow checks if a log line arriving at `now` is allowed under the rate limit.
func (rl *SlidingWindowRateLimiter) Allow(now time.Time) bool {
	cutoff := now.Add(-rl.WindowDuration)

	// Prune timestamps older than cutoff
	validIdx := 0
	for validIdx < len(rl.timestamps) && rl.timestamps[validIdx].Before(cutoff) {
		validIdx++
	}
	if validIdx > 0 {
		rl.timestamps = rl.timestamps[validIdx:]
	}

	if len(rl.timestamps) >= rl.MaxLinesPerWindow {
		return false // Rate limited / dropped
	}

	rl.timestamps = append(rl.timestamps, now)
	return true
}

// FilterStream processes a slice of log lines with simulated or extracted timestamps,
// returning only the permitted lines and the count of suppressed lines.
func (rl *SlidingWindowRateLimiter) FilterStream(lines []string, baseTime time.Time, interval time.Duration) ([]string, int) {
	var allowed []string
	suppressed := 0

	curTime := baseTime
	for _, line := range lines {
		if rl.Allow(curTime) {
			allowed = append(allowed, line)
		} else {
			suppressed++
		}
		curTime = curTime.Add(interval)
	}

	return allowed, suppressed
}

// FormatRateLimitStats formats rate limiting summary statistics.
func FormatRateLimitStats(totalLines, allowedCount, suppressedCount int) string {
	return fmt.Sprintf("Total: %d lines (Allowed: %d, Suppressed: %d due to rate limit)",
		totalLines, allowedCount, suppressedCount)
}
