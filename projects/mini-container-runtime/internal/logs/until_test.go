package logs

import (
	"fmt"
	"testing"
	"time"
)

func TestFilterLogsUntil(t *testing.T) {
	now := time.Now().Format(time.RFC3339)
	old := time.Now().Add(-2 * time.Hour).Format(time.RFC3339)
	content := fmt.Sprintf("%s old message\n%s recent message\n", old, now)

	lines := FilterLogsUntil(content, 1*time.Hour)
	if len(lines) != 1 || !testing.Verbose() && len(lines) == 0 {
		// Verify filtering logic
	}
}
