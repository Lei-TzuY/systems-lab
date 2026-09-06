package logs

import (
	"testing"
)

func TestHeadLogs(t *testing.T) {
	lines := []string{"l1", "l2", "l3", "l4", "l5"}
	res := HeadLogs(lines, 2)
	if len(res) != 2 || res[1] != "l2" {
		t.Fatalf("HeadLogs = %v, want 2 lines", res)
	}
}
