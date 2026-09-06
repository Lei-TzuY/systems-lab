package logs

import (
	"strings"
	"testing"
)

func TestAttachLogDetails(t *testing.T) {
	content := "app server initialized\nready for traffic\n"
	details := map[string]string{"env": "prod", "id": "ctr-123"}

	res := AttachLogDetails(content, details)
	if !strings.Contains(res, "env=prod") || !strings.Contains(res, "app server initialized") {
		t.Fatalf("AttachLogDetails = %s, want detailed log formatting", res)
	}
}
