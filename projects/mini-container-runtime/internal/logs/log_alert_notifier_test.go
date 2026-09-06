package logs

import (
	"strings"
	"sync"
	"testing"
	"time"
)

func TestLogAlertEngine_DefaultRules(t *testing.T) {
	engine := NewDefaultAlertEngine()

	lines := []string{
		"2026-08-22T00:01:00Z [INFO] Application booted",
		"2026-08-22T00:01:05Z [FATAL] panic: runtime error: invalid memory address",
		"2026-08-22T00:01:10Z [CRITICAL] kernel: oom-killer invoked on pid 42",
		"2026-08-22T00:01:15Z [INFO] shutting down gracefully",
	}

	events := engine.ScanStream(lines, time.Time{})
	if len(events) != 2 {
		t.Fatalf("expected 2 alert events, got %d", len(events))
	}

	if events[0].TriggerName != "Panic Detected" {
		t.Errorf("events[0].TriggerName = %q, want Panic Detected", events[0].TriggerName)
	}
	expectedTime1, _ := time.Parse(time.RFC3339, "2026-08-22T00:01:05Z")
	if !events[0].Timestamp.Equal(expectedTime1) {
		t.Errorf("events[0].Timestamp = %v, want %v", events[0].Timestamp, expectedTime1)
	}

	if events[1].TriggerName != "OOM Killer" {
		t.Errorf("events[1].TriggerName = %q, want OOM Killer", events[1].TriggerName)
	}
}

func TestLogAlertEngine_AddRuleValidation(t *testing.T) {
	engine := NewDefaultAlertEngine()

	if err := engine.AddRule("", "pattern", "INFO"); err == nil {
		t.Fatal("expected error for empty rule name")
	}
	if err := engine.AddRule("Name", "", "INFO"); err == nil {
		t.Fatal("expected error for empty rule pattern")
	}
	if err := engine.AddRule("BadRegex", "[unclosed", "INFO"); err == nil {
		t.Fatal("expected error for invalid regex pattern")
	}
}

func TestLogAlertEngine_ConcurrentSafety(t *testing.T) {
	engine := NewDefaultAlertEngine()

	var wg sync.WaitGroup
	for i := 0; i < 20; i++ {
		wg.Add(2)
		go func(id int) {
			defer wg.Done()
			_ = engine.AddRule("CustomRule", `(?i)custom-error`, "ERROR")
		}(i)
		go func() {
			defer wg.Done()
			_ = engine.ScanStream([]string{"2026-08-22T00:00:00Z [ERROR] custom-error found"}, time.Time{})
		}()
	}
	wg.Wait()

	if engine.RulesCount() < 4 {
		t.Errorf("RulesCount = %d, want at least 4", engine.RulesCount())
	}
}

func TestFormatAlertSummary(t *testing.T) {
	events := []AlertEvent{
		{Timestamp: time.Now(), TriggerName: "OOM Killer", MatchedLine: "out of memory", Severity: "CRITICAL"},
	}
	got := FormatAlertSummary(events)
	if !strings.Contains(got, "Alerts Detected: 1 events") {
		t.Errorf("expected header in %q", got)
	}
	if !strings.Contains(got, "[CRITICAL] OOM Killer") {
		t.Errorf("expected trigger in %q", got)
	}
}

func TestFormatAlertSummary_Empty(t *testing.T) {
	got := FormatAlertSummary(nil)
	if got != "Alerts: (none detected)" {
		t.Errorf("got %q, want 'Alerts: (none detected)'", got)
	}
}
