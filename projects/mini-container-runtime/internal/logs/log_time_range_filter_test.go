package logs

import (
	"reflect"
	"testing"
	"time"
)

func TestTimeRangeFilter_Match(t *testing.T) {
	t1 := time.Date(2026, 8, 20, 10, 0, 0, 0, time.UTC)
	t2 := time.Date(2026, 8, 20, 12, 0, 0, 0, time.UTC)

	filter := NewTimeRangeFilter(&t1, &t2)

	tests := []struct {
		name     string
		line     string
		expected bool
	}{
		{
			name:     "within window rfc3339",
			line:     "2026-08-20T11:00:00Z [INFO] job running",
			expected: true,
		},
		{
			name:     "before window rfc3339",
			line:     "2026-08-20T09:30:00Z [INFO] early job",
			expected: false,
		},
		{
			name:     "after window rfc3339",
			line:     "2026-08-20T13:00:00Z [INFO] late job",
			expected: false,
		},
		{
			name:     "within window space format",
			line:     "2026-08-20 11:30:00 [WARN] warning",
			expected: true,
		},
		{
			name:     "no timestamp passes through",
			line:     "just plain log line",
			expected: true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := filter.Match(tc.line)
			if got != tc.expected {
				t.Errorf("Match(%q) = %t, want %t", tc.line, got, tc.expected)
			}
		})
	}
}

func TestTimeRangeFilter_FilterLines(t *testing.T) {
	t1 := time.Date(2026, 8, 20, 10, 0, 0, 0, time.UTC)
	filter := NewTimeRangeFilter(&t1, nil)

	lines := []string{
		"2026-08-20T08:00:00Z line1",
		"2026-08-20T11:00:00Z line2",
		"2026-08-20T12:00:00Z line3",
	}

	got := filter.FilterLines(lines)
	want := []string{
		"2026-08-20T11:00:00Z line2",
		"2026-08-20T12:00:00Z line3",
	}

	if !reflect.DeepEqual(got, want) {
		t.Errorf("FilterLines() = %v, want %v", got, want)
	}
}
