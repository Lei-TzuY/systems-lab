// Package logs provides container log processing utilities.
// This file implements a time-window aggregation engine that groups
// timestamped container logs into fixed duration buckets and calculates metrics.

package logs

import (
	"fmt"
	"sort"
	"strings"
	"time"
)

// LogWindowBucket holds aggregate metrics for a single time slice.
type LogWindowBucket struct {
	StartTime  time.Time
	EndTime    time.Time
	TotalLines int
	ErrorCount int
	WarnCount  int
}

// LogTimeWindowGrouper aggregates log lines into fixed duration time windows.
type LogTimeWindowGrouper struct {
	WindowDuration time.Duration
}

// NewLogTimeWindowGrouper creates a LogTimeWindowGrouper.
func NewLogTimeWindowGrouper(windowDuration time.Duration) *LogTimeWindowGrouper {
	if windowDuration <= 0 {
		windowDuration = time.Minute
	}
	return &LogTimeWindowGrouper{
		WindowDuration: windowDuration,
	}
}

// GroupLines aggregates log lines into sorted time buckets with nanosecond precision.
func (g *LogTimeWindowGrouper) GroupLines(lines []string, fallbackTime time.Time) []LogWindowBucket {
	if fallbackTime.IsZero() {
		fallbackTime = time.Now().UTC()
	} else {
		fallbackTime = fallbackTime.UTC()
	}

	buckets := make(map[int64]*LogWindowBucket)

	for _, line := range lines {
		t, ok := ExtractTimestamp(line)
		if !ok {
			t = fallbackTime
		} else {
			t = t.UTC()
		}

		windowStart := t.Truncate(g.WindowDuration)
		windowStartNano := windowStart.UnixNano()
		b, exists := buckets[windowStartNano]
		if !exists {
			b = &LogWindowBucket{
				StartTime: windowStart,
				EndTime:   windowStart.Add(g.WindowDuration),
			}
			buckets[windowStartNano] = b
		}

		b.TotalLines++
		upper := strings.ToUpper(line)
		if strings.Contains(upper, "ERROR") || strings.Contains(upper, "FATAL") || strings.Contains(upper, "PANIC") {
			b.ErrorCount++
		} else if strings.Contains(upper, "WARN") {
			b.WarnCount++
		}
	}

	var result []LogWindowBucket
	for _, b := range buckets {
		result = append(result, *b)
	}

	sort.Slice(result, func(i, j int) bool {
		return result[i].StartTime.Before(result[j].StartTime)
	})

	return result
}

// FormatWindowBuckets renders an ASCII histogram of log volume per time window.
func FormatWindowBuckets(buckets []LogWindowBucket) string {
	if len(buckets) == 0 {
		return "Log Windows: (no data)"
	}

	timeFormat := "15:04:05"
	// Use millisecond precision if any bucket has sub-second start time
	for _, b := range buckets {
		if b.StartTime.Nanosecond() != 0 {
			timeFormat = "15:04:05.000"
			break
		}
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("Log Window Distribution (%d windows):\n", len(buckets)))
	for _, b := range buckets {
		barLen := b.TotalLines
		if barLen > 30 {
			barLen = 30
		}
		bar := strings.Repeat("█", barLen)
		sb.WriteString(fmt.Sprintf("  [%s] %4d lines (errors: %d, warns: %d) %s\n",
			b.StartTime.Format(timeFormat), b.TotalLines, b.ErrorCount, b.WarnCount, bar))
	}
	return strings.TrimRight(sb.String(), "\n")
}
