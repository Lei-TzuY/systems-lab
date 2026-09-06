package logs

import (
	"fmt"
)

// LimitLogBytes truncates log content to maximum byte length.
func LimitLogBytes(content string, maxBytes int) string {
	if maxBytes <= 0 || len(content) <= maxBytes {
		return content
	}

	truncated := content[:maxBytes]
	return fmt.Sprintf("%s\n[Truncated after %d bytes]", truncated, maxBytes)
}
