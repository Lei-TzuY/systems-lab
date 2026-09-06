package logs

import (
	"strings"
	"testing"
	"time"
)

func TestSlidingWindowRateLimiter_Allow(t *testing.T) {
	limiter := NewSlidingWindowRateLimiter(3, time.Second)
	now := time.Date(2026, 8, 20, 12, 0, 0, 0, time.UTC)

	// First 3 lines within 1s should be allowed
	if !limiter.Allow(now) {
		t.Error("expected 1st line allowed")
	}
	if !limiter.Allow(now.Add(100 * time.Millisecond)) {
		t.Error("expected 2nd line allowed")
	}
	if !limiter.Allow(now.Add(200 * time.Millisecond)) {
		t.Error("expected 3rd line allowed")
	}

	// 4th line within the same second should be throttled
	if limiter.Allow(now.Add(300 * time.Millisecond)) {
		t.Error("expected 4th line throttled")
	}

	// After 1.1 seconds, new lines should be allowed as window slides
	if !limiter.Allow(now.Add(1100 * time.Millisecond)) {
		t.Error("expected line allowed after window slide")
	}
}

func TestSlidingWindowRateLimiter_FilterStream(t *testing.T) {
	limiter := NewSlidingWindowRateLimiter(2, time.Second)
	lines := []string{"msg 1", "msg 2", "msg 3", "msg 4"}
	base := time.Now()

	// 4 messages within 100ms each (total 300ms < 1s window)
	allowed, suppressed := limiter.FilterStream(lines, base, 100*time.Millisecond)

	if len(allowed) != 2 {
		t.Errorf("len(allowed) = %d, want 2", len(allowed))
	}
	if suppressed != 2 {
		t.Errorf("suppressed = %d, want 2", suppressed)
	}

	stats := FormatRateLimitStats(len(lines), len(allowed), suppressed)
	if !strings.Contains(stats, "Suppressed: 2") {
		t.Errorf("expected 'Suppressed: 2' in stats, got %q", stats)
	}
}
