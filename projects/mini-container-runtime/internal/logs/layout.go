package logs

import (
	"strings"
)

// RenderLogLayout formats log lines using a layout template with placeholders ({time}, {msg}).
func RenderLogLayout(logContent, layoutTemplate string) string {
	if layoutTemplate == "" {
		return logContent
	}

	lines := strings.Split(strings.TrimRight(logContent, "\n"), "\n")
	var sb strings.Builder

	for _, line := range lines {
		if line == "" {
			continue
		}
		parts := strings.SplitN(line, " ", 2)
		lineTime := ""
		lineMsg := line

		if len(parts) == 2 && strings.Contains(parts[0], "T") {
			lineTime = parts[0]
			lineMsg = parts[1]
		}

		formatted := strings.ReplaceAll(layoutTemplate, "{time}", lineTime)
		formatted = strings.ReplaceAll(formatted, "{msg}", lineMsg)
		sb.WriteString(formatted + "\n")
	}

	return sb.String()
}
