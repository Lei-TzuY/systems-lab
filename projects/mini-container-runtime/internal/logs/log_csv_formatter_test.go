package logs

import (
	"strings"
	"testing"
)

func TestCSVLogFormatter_FormatLines(t *testing.T) {
	lines := []string{
		"2026-08-20T12:00:00Z [INFO] Service started successfully",
		"2026-08-20T12:05:00Z [ERROR] Failed to query database: \"timeout\"",
	}

	formatter := NewCSVLogFormatter(',', true, []string{"time", "level", "message"})
	csvOutput, err := formatter.FormatLines(lines)
	if err != nil {
		t.Fatalf("FormatLines failed: %v", err)
	}

	if !strings.HasPrefix(csvOutput, "time,level,message\n") {
		t.Errorf("expected header row in CSV output, got:\n%s", csvOutput)
	}

	if !strings.Contains(csvOutput, "2026-08-20T12:00:00Z,INFO,") {
		t.Errorf("expected formatted row in CSV, got:\n%s", csvOutput)
	}

	// Verify quotes escaping
	if !strings.Contains(csvOutput, "\"\"timeout\"\"") {
		t.Errorf("expected escaped quotes in CSV output, got:\n%s", csvOutput)
	}
}

func TestCSVLogFormatter_TSV(t *testing.T) {
	lines := []string{
		"2026-08-20 [INFO] test",
	}

	formatter := NewCSVLogFormatter('\t', false, nil)
	tsvOutput, err := formatter.FormatLines(lines)
	if err != nil {
		t.Fatalf("FormatLines TSV failed: %v", err)
	}

	if strings.Contains(tsvOutput, "timestamp") {
		t.Errorf("expected no header in output, got %s", tsvOutput)
	}
	if !strings.Contains(tsvOutput, "\tINFO\t") {
		t.Errorf("expected tab separators in TSV, got %s", tsvOutput)
	}
}
