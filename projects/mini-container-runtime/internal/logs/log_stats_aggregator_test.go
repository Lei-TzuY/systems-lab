package logs

import (
	"strings"
	"testing"
)

func TestLogStatsAggregator_ProcessLines(t *testing.T) {
	lines := []string{
		"2026-08-20 [DEBUG] cache hit",
		"2026-08-20 [INFO] request handled",
		"2026-08-20 [WARN] memory threshold reached",
		"2026-08-20 [ERROR] failed to connect",
		"2026-08-20 [FATAL] panic exit",
		"unformatted standard output",
		"",
	}

	agg := NewLogStatsAggregator()
	stats := agg.ProcessLines(lines)

	if stats.TotalLines != 7 {
		t.Errorf("TotalLines = %d, want 7", stats.TotalLines)
	}
	if stats.EmptyLines != 1 {
		t.Errorf("EmptyLines = %d, want 1", stats.EmptyLines)
	}
	if stats.NonEmptyLines != 6 {
		t.Errorf("NonEmptyLines = %d, want 6", stats.NonEmptyLines)
	}
	if stats.DebugCount != 1 {
		t.Errorf("DebugCount = %d, want 1", stats.DebugCount)
	}
	if stats.InfoCount != 1 {
		t.Errorf("InfoCount = %d, want 1", stats.InfoCount)
	}
	if stats.WarnCount != 1 {
		t.Errorf("WarnCount = %d, want 1", stats.WarnCount)
	}
	if stats.ErrorCount != 1 {
		t.Errorf("ErrorCount = %d, want 1", stats.ErrorCount)
	}
	if stats.FatalCount != 1 {
		t.Errorf("FatalCount = %d, want 1", stats.FatalCount)
	}
	if stats.UnknownCount != 2 {
		t.Errorf("UnknownCount = %d, want 2 (one text line + one empty)", stats.UnknownCount)
	}
	if stats.TotalBytes <= 0 {
		t.Errorf("expected TotalBytes > 0, got %d", stats.TotalBytes)
	}
	if stats.AvgLineLength <= 0 {
		t.Errorf("expected AvgLineLength > 0, got %f", stats.AvgLineLength)
	}
}

func TestLogStatsAggregator_FormatStats(t *testing.T) {
	agg := NewLogStatsAggregator()
	agg.ProcessLine("2026-08-20 [INFO] ready")

	formatted := agg.FormatStats()
	if !strings.Contains(formatted, "Total Lines:    1") {
		t.Errorf("expected Total Lines: 1 in %q", formatted)
	}
	if !strings.Contains(formatted, "INFO=1") {
		t.Errorf("expected INFO=1 in %q", formatted)
	}
}
