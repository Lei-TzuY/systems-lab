package logs

import (
	"testing"
)

func TestPrefixLogs(t *testing.T) {
	lines := []string{"hello", "world"}
	prefixed := PrefixLogs(lines, "[app] ")
	if len(prefixed) != 2 || prefixed[0] != "[app] hello" || prefixed[1] != "[app] world" {
		t.Fatalf("PrefixLogs = %v, want prefixed lines", prefixed)
	}
}
