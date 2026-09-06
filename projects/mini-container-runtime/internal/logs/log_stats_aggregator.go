// Package logs provides container log processing utilities.
// This file implements a stream summary statistics aggregator that computes metrics
// such as line counts, byte sizes, average line length, and log level counts.

package logs

import (
	"fmt"
	"strings"
)

// LogStreamStats holds aggregate metrics computed from a container log stream.
type LogStreamStats struct {
	TotalLines    int
	EmptyLines    int
	NonEmptyLines int
	TotalBytes    int64
	AvgLineLength float64
	DebugCount    int
	InfoCount     int
	WarnCount     int
	ErrorCount    int
	FatalCount    int
	UnknownCount  int
}

// LogStatsAggregator aggregates summary metrics over log lines.
type LogStatsAggregator struct {
	stats       LogStreamStats
	levelFilter *LogLevelFilter
}

// NewLogStatsAggregator creates a new statistics aggregator.
func NewLogStatsAggregator() *LogStatsAggregator {
	return &LogStatsAggregator{
		levelFilter: NewLogLevelFilter("debug"),
	}
}

// ProcessLine processes a single log line and updates aggregate metrics.
func (a *LogStatsAggregator) ProcessLine(line string) {
	a.stats.TotalLines++
	lineBytes := int64(len(line))
	a.stats.TotalBytes += lineBytes

	if strings.TrimSpace(line) == "" {
		a.stats.EmptyLines++
	} else {
		a.stats.NonEmptyLines++
	}

	sev := a.levelFilter.DetectSeverity(line)
	switch sev {
	case SeverityDebug:
		a.stats.DebugCount++
	case SeverityInfo:
		a.stats.InfoCount++
	case SeverityWarn:
		a.stats.WarnCount++
	case SeverityError:
		a.stats.ErrorCount++
	case SeverityFatal:
		a.stats.FatalCount++
	default:
		a.stats.UnknownCount++
	}
}

// ProcessLines processes multiple log lines in batch.
func (a *LogStatsAggregator) ProcessLines(lines []string) LogStreamStats {
	for _, line := range lines {
		a.ProcessLine(line)
	}
	return a.Stats()
}

// Stats returns the computed summary statistics.
func (a *LogStatsAggregator) Stats() LogStreamStats {
	result := a.stats
	if result.TotalLines > 0 {
		result.AvgLineLength = float64(result.TotalBytes) / float64(result.TotalLines)
	}
	return result
}

// FormatStats returns a human-readable multi-line summary of the log stream metrics.
func (a *LogStatsAggregator) FormatStats() string {
	s := a.Stats()
	return fmt.Sprintf("Log Stream Statistics:\n"+
		"  Total Lines:    %d (Non-Empty: %d, Empty: %d)\n"+
		"  Total Volume:   %d bytes (Avg Length: %.1f chars)\n"+
		"  Level Breakdown: DEBUG=%d, INFO=%d, WARN=%d, ERROR=%d, FATAL=%d, OTHER=%d",
		s.TotalLines, s.NonEmptyLines, s.EmptyLines,
		s.TotalBytes, s.AvgLineLength,
		s.DebugCount, s.InfoCount, s.WarnCount, s.ErrorCount, s.FatalCount, s.UnknownCount)
}
