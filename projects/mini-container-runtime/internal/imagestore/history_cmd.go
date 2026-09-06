package imagestore

import (
	"strings"
)

// CleanHistoryCommand parses created_by strings into clean build instructions.
func CleanHistoryCommand(rawCreatedBy string) string {
	cleaned := strings.TrimSpace(rawCreatedBy)
	cleaned = strings.TrimPrefix(cleaned, "/bin/sh -c #(nop) ")
	cleaned = strings.TrimPrefix(cleaned, "/bin/sh -c ")
	return strings.TrimSpace(cleaned)
}
