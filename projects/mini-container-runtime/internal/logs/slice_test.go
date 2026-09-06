package logs

import (
	"testing"
)

func TestSliceLogs(t *testing.T) {
	lines := []string{"l0", "l1", "l2", "l3", "l4"}
	sliced := SliceLogs(lines, 1, 3)
	if len(sliced) != 3 || sliced[0] != "l1" || sliced[2] != "l3" {
		t.Fatalf("SliceLogs = %v, want [l1 l2 l3]", sliced)
	}
}
