package logs

import (
	"strings"
	"testing"
	"time"
)

func TestFormatDelta(t *testing.T) {
	tests := []struct {
		d    time.Duration
		want string
	}{
		{500 * time.Microsecond, "[+500µs]"},
		{15 * time.Millisecond, "[+15.0ms]"},
		{2500 * time.Millisecond, "[+2.50s]"},
	}

	for _, tc := range tests {
		t.Run(tc.want, func(t *testing.T) {
			got := FormatDelta(tc.d)
			if got != tc.want {
				t.Errorf("FormatDelta(%v) = %q, want %q", tc.d, got, tc.want)
			}
		})
	}
}

func TestLogDeltaTimer_AnnotateLines(t *testing.T) {
	lines := []string{
		"2026-08-20T12:00:00.000Z [INFO] init step 1",
		"2026-08-20T12:00:00.050Z [INFO] init step 2",
		"2026-08-20T12:00:01.550Z [INFO] db connect done",
	}

	timer := NewLogDeltaTimer(0)
	annotated := timer.AnnotateLines(lines)

	if !strings.HasPrefix(annotated[0], "[+0.0ms]") {
		t.Errorf("expected [+0.0ms] on line 0, got %q", annotated[0])
	}
	if !strings.HasPrefix(annotated[1], "[+50.0ms]") {
		t.Errorf("expected [+50.0ms] on line 1, got %q", annotated[1])
	}
	if !strings.HasPrefix(annotated[2], "[+1.50s]") {
		t.Errorf("expected [+1.50s] on line 2, got %q", annotated[2])
	}
}
