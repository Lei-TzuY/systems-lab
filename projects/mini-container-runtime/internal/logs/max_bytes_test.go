package logs

import (
	"strings"
	"testing"
)

func TestLimitLogBytes(t *testing.T) {
	content := "1234567890abcdefghij"
	res := LimitLogBytes(content, 10)
	if !strings.Contains(res, "1234567890") || !strings.Contains(res, "Truncated after 10 bytes") {
		t.Fatalf("LimitLogBytes = %s, want truncated output", res)
	}
}
