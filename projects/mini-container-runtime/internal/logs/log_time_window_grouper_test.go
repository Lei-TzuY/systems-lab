package logs

import (
	"strings"
	"testing"
	"time"
)

func TestLogTimeWindowGrouper_GroupLines(t *testing.T) {
	lines := []string{
		"2026-08-21T10:00:05Z [INFO] Service starting",
		"2026-08-21T10:00:25Z [WARN] Memory high",
		"2026-08-21T10:00:55Z [ERROR] Failed to connect DB",
		"2026-08-21T10:01:10Z [INFO] Retrying connection",
		"2026-08-21T10:01:20Z [INFO] Connected successfully",
	}

	grouper := NewLogTimeWindowGrouper(time.Minute)
	buckets := grouper.GroupLines(lines, time.Time{})

	if len(buckets) != 2 {
		t.Fatalf("expected 2 minute buckets, got %d", len(buckets))
	}

	// First bucket: 10:00:00 - 10:01:00
	if buckets[0].TotalLines != 3 {
		t.Errorf("bucket[0].TotalLines = %d, want 3", buckets[0].TotalLines)
	}
	if buckets[0].ErrorCount != 1 {
		t.Errorf("bucket[0].ErrorCount = %d, want 1", buckets[0].ErrorCount)
	}
	if buckets[0].WarnCount != 1 {
		t.Errorf("bucket[0].WarnCount = %d, want 1", buckets[0].WarnCount)
	}

	// Second bucket: 10:01:00 - 10:02:00
	if buckets[1].TotalLines != 2 {
		t.Errorf("bucket[1].TotalLines = %d, want 2", buckets[1].TotalLines)
	}
	if buckets[1].ErrorCount != 0 {
		t.Errorf("bucket[1].ErrorCount = %d, want 0", buckets[1].ErrorCount)
	}
}

func TestLogTimeWindowGrouper_SubsecondBucketing(t *testing.T) {
	lines := []string{
		"2026-08-21T10:00:00.050Z [INFO] First subsecond event",
		"2026-08-21T10:00:00.080Z [INFO] Second subsecond event in same 100ms bucket",
		"2026-08-21T10:00:00.250Z [ERROR] Event in 200ms bucket",
	}

	grouper := NewLogTimeWindowGrouper(100 * time.Millisecond)
	buckets := grouper.GroupLines(lines, time.Time{})

	if len(buckets) != 2 {
		t.Fatalf("expected 2 sub-second buckets, got %d", len(buckets))
	}

	if buckets[0].TotalLines != 2 {
		t.Errorf("bucket[0].TotalLines = %d, want 2", buckets[0].TotalLines)
	}
	if buckets[1].TotalLines != 1 {
		t.Errorf("bucket[1].TotalLines = %d, want 1", buckets[1].TotalLines)
	}
	if buckets[1].ErrorCount != 1 {
		t.Errorf("bucket[1].ErrorCount = %d, want 1", buckets[1].ErrorCount)
	}

	formatted := FormatWindowBuckets(buckets)
	if !strings.Contains(formatted, "10:00:00.000") {
		t.Errorf("expected millisecond time format in %q", formatted)
	}
}

func TestFormatWindowBuckets_Empty(t *testing.T) {
	got := FormatWindowBuckets(nil)
	if got != "Log Windows: (no data)" {
		t.Errorf("got %q, want 'Log Windows: (no data)'", got)
	}
}
