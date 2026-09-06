package imagestore

import (
	"strings"
)

// IsPlatformCompatible checks if manifest OS and Arch match target OS/Arch.
func IsPlatformCompatible(manifestOS, manifestArch, targetOS, targetArch string) bool {
	if manifestOS == "" {
		manifestOS = "linux"
	}
	if manifestArch == "" {
		manifestArch = "amd64"
	}
	return strings.EqualFold(manifestOS, targetOS) && strings.EqualFold(manifestArch, targetArch)
}
