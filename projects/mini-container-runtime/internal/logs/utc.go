package logs

import (
	"fmt"
	"strings"
	"time"
)

// ConvertLogsToUTC converts log timestamps to UTC format.
func ConvertLogsToUTC(logContent string) string {
	lines := strings.Split(strings.TrimRight(logContent, "\n"), "\n")
	var sb strings.Builder

	for _, line := range lines {
		parts := strings.SplitN(line, " ", 2)
		if len(parts) == 2 {
			if t, err := time.Parse(time.RFC3339, parts[0]); err == nil {
				utcStr := t.UTC().Format(time.RFC3339)
				sb.WriteString(fmt.Sprintf("%s %s\n", utcStr, parts[1]))
				continue
			}
		}
		sb.WriteString(line + "\n")
	}

	return sb.String()
}
