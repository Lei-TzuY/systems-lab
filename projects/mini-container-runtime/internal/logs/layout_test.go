package logs

import (
	"strings"
	"testing"
)

func TestRenderLogLayout(t *testing.T) {
	content := "2026-08-15T12:00:00Z system ready\n"
	tmpl := "[LOG] {time} -> {msg}"

	res := RenderLogLayout(content, tmpl)
	if !strings.Contains(res, "[LOG] 2026-08-15T12:00:00Z -> system ready") {
		t.Fatalf("RenderLogLayout = %s, want template rendered output", res)
	}
}
