package container

import (
	"fmt"
	"strings"
)

var sensitiveKeywords = []string{
	"PASS", "PASSWORD", "SECRET", "KEY", "TOKEN", "CREDENTIAL", "AUTH", "PRIVATE",
}

// MaskEnvVars returns a copy of environment variables with sensitive values obscured.
func MaskEnvVars(env []string) []string {
	masked := make([]string, len(env))
	for i, entry := range env {
		parts := strings.SplitN(entry, "=", 2)
		if len(parts) == 2 {
			keyUpper := strings.ToUpper(parts[0])
			isSensitive := false
			for _, kw := range sensitiveKeywords {
				if strings.Contains(keyUpper, kw) {
					isSensitive = true
					break
				}
			}
			if isSensitive && len(parts[1]) > 0 {
				masked[i] = fmt.Sprintf("%s=******", parts[0])
				continue
			}
		}
		masked[i] = entry
	}
	return masked
}
