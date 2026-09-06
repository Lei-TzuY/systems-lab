package logs

import (
	"fmt"
	"strings"
)

// AttachLogDetails formats log entries with attached key-value details metadata.
func AttachLogDetails(logContent string, details map[string]string) string {
	if len(details) == 0 {
		return logContent
	}

	var detailParts []string
	for k, v := range details {
		detailParts = append(detailParts, fmt.Sprintf("%s=%s", k, v))
	}
	prefix := strings.Join(detailParts, ",")

	lines := strings.Split(strings.TrimRight(logContent, "\n"), "\n")
	var sb strings.Builder
	for _, line := range lines {
		if line != "" {
			sb.WriteString(fmt.Sprintf("[%s] %s\n", prefix, line))
		}
	}

	return sb.String()
}
