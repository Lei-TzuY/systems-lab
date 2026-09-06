package logs

import (
	"testing"
)

func TestFilterLogRate(t *testing.T) {
	lines := []string{"line1", "line2", "line3", "line4", "line5"}
	filtered := FilterLogRate(lines, 3)
	if len(filtered) != 3 {
		t.Fatalf("FilterLogRate len = %d, want 3", len(filtered))
	}
}
