package logs

import (
	"strings"
	"testing"
)

func TestColorizeLogLine(t *testing.T) {
	tests := []struct {
		input     string
		wantColor string
	}{
		{"[FATAL] process crashed", ColorBoldRed},
		{"[ERROR] connection timeout", ColorRed},
		{"[WARN] high memory", ColorYellow},
		{"[INFO] server started", ColorGreen},
		{"[DEBUG] trace payload", ColorGray},
		{"plain text line", ""},
	}

	for _, tc := range tests {
		t.Run(tc.input, func(t *testing.T) {
			got := ColorizeLogLine(tc.input)
			if tc.wantColor == "" {
				if got != tc.input {
					t.Errorf("plain line should be unchanged, got %q", got)
				}
			} else {
				if !strings.HasPrefix(got, tc.wantColor) {
					limit := len(got)
					if limit > 10 {
						limit = 10
					}
					t.Errorf("expected prefix %q, got %q", tc.wantColor, got[:limit])
				}
				if !strings.HasSuffix(got, ColorReset) {
					t.Errorf("expected suffix %q", ColorReset)
				}
			}
		})
	}
}

func TestStripANSI(t *testing.T) {
	colored := ColorBoldRed + "[FATAL] crash" + ColorReset
	stripped := StripANSI(colored)
	if stripped != "[FATAL] crash" {
		t.Errorf("StripANSI = %q, want %q", stripped, "[FATAL] crash")
	}
}

func TestColorizeLogStream(t *testing.T) {
	lines := []string{
		"[INFO] started",
		"[ERROR] failed",
		"plain",
	}
	out := ColorizeLogStream(lines)
	if len(out) != 3 {
		t.Fatalf("expected 3 lines, got %d", len(out))
	}
	if !strings.Contains(out[0], ColorGreen) {
		t.Errorf("line 0 should be green")
	}
	if !strings.Contains(out[1], ColorRed) {
		t.Errorf("line 1 should be red")
	}
	if out[2] != "plain" {
		t.Errorf("line 2 should be unchanged")
	}
}
