package imagestore

import (
	"testing"
)

func TestIsPlatformCompatible(t *testing.T) {
	if !IsPlatformCompatible("linux", "amd64", "linux", "amd64") {
		t.Fatalf("IsPlatformCompatible linux/amd64 = false, want true")
	}
	if IsPlatformCompatible("windows", "amd64", "linux", "amd64") {
		t.Fatalf("IsPlatformCompatible windows/amd64 vs linux/amd64 = true, want false")
	}
}
