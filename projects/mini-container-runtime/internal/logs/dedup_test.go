package logs

import (
	"strings"
	"testing"
)

func TestDeduplicateLogs(t *testing.T) {
	lines := []string{"hello", "hello", "hello", "world"}
	res := DeduplicateLogs(lines)
	if len(res) != 3 || !strings.Contains(res[1], "repeated 2 times") {
		t.Fatalf("DeduplicateLogs = %v, want repeated summary", res)
	}
}
