package logs

import (
	"strings"
	"testing"
)

func TestTruncateFilter_Truncate(t *testing.T) {
	tf := NewTruncateFilter(20, "...")

	tests := []struct {
		name  string
		input string
		want  string
	}{
		{
			name:  "short line unchanged",
			input: "hello",
			want:  "hello",
		},
		{
			name:  "exact length unchanged",
			input: strings.Repeat("a", 20),
			want:  strings.Repeat("a", 20),
		},
		{
			name:  "long line truncated",
			input: strings.Repeat("b", 30),
			want:  strings.Repeat("b", 17) + "...",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := tf.Truncate(tc.input)
			if got != tc.want {
				t.Errorf("got %q (len %d), want %q (len %d)", got, len(got), tc.want, len(tc.want))
			}
		})
	}
}

func TestTruncateFilter_FilterLines(t *testing.T) {
	tf := NewTruncateFilter(10, "...")
	lines := []string{"short", strings.Repeat("x", 20)}
	got := tf.FilterLines(lines)
	if got[0] != "short" {
		t.Errorf("expected short line unchanged, got %q", got[0])
	}
	if len(got[1]) > 10 {
		t.Errorf("expected truncated to <=10 bytes, got len %d", len(got[1]))
	}
}

func TestTruncateFilter_Defaults(t *testing.T) {
	tf := NewTruncateFilter(0, "")
	if tf.MaxBytes != 4096 {
		t.Errorf("expected default 4096, got %d", tf.MaxBytes)
	}
	if tf.Suffix != "...[truncated]" {
		t.Errorf("expected default suffix, got %q", tf.Suffix)
	}
}

func TestFormatTruncateStats(t *testing.T) {
	lines := []string{"short", strings.Repeat("x", 100)}
	got := FormatTruncateStats(lines, 50)
	if !strings.Contains(got, "Truncated: 1") {
		t.Errorf("expected 1 truncated, got %q", got)
	}
}
