package system

import (
	"runtime"
	"testing"
)

func TestCheckKernelFeatures(t *testing.T) {
	res := CheckKernelFeatures()
	if res.OS != runtime.GOOS {
		t.Fatalf("OS = %s, want %s", res.OS, runtime.GOOS)
	}
	if res.Arch != runtime.GOARCH {
		t.Fatalf("Arch = %s, want %s", res.Arch, runtime.GOARCH)
	}
}
