// Package logs provides container log processing utilities.
// This file implements a step duration and latency annotator that computes
// elapsed time differences (+delta) between consecutive log timestamps.

package logs

import (
	"fmt"
	"time"
)

// LogDeltaTimer computes and prepends step duration offsets (+delta) between consecutive logs.
type LogDeltaTimer struct {
	lastTime  time.Time
	hasFirst  bool
	Threshold time.Duration // Only highlight deltas larger than Threshold if non-zero
}

// NewLogDeltaTimer creates a LogDeltaTimer.
func NewLogDeltaTimer(threshold time.Duration) *LogDeltaTimer {
	return &LogDeltaTimer{
		Threshold: threshold,
	}
}

// FormatDelta formats a duration into a compact prefix like [+12.5ms] or [+1.20s].
func FormatDelta(d time.Duration) string {
	if d < 0 {
		d = 0
	}
	if d < time.Millisecond {
		return fmt.Sprintf("[+%dµs]", d.Microseconds())
	}
	if d < time.Second {
		return fmt.Sprintf("[+%.1fms]", float64(d.Microseconds())/1000.0)
	}
	return fmt.Sprintf("[+%.2fs]", d.Seconds())
}

// AnnotateLine extracts the timestamp from line, calculates delta against previous line,
// and returns the line prefixed with the delta tag.
func (dt *LogDeltaTimer) AnnotateLine(line string) string {
	t, ok := ExtractTimestamp(line)
	if !ok {
		return line
	}

	if !dt.hasFirst {
		dt.lastTime = t
		dt.hasFirst = true
		return fmt.Sprintf("[+0.0ms] %s", line)
	}

	delta := t.Sub(dt.lastTime)
	dt.lastTime = t

	if dt.Threshold > 0 && delta < dt.Threshold {
		return line
	}

	return fmt.Sprintf("%s %s", FormatDelta(delta), line)
}

// AnnotateLines processes a slice of log lines, annotating each with inter-line latency.
func (dt *LogDeltaTimer) AnnotateLines(lines []string) []string {
	out := make([]string, len(lines))
	for i, line := range lines {
		out[i] = dt.AnnotateLine(line)
	}
	return out
}
