//go:build !linux

package container

import (
	"errors"
	"testing"
)

func TestRunPayloadExitCodeUnsupportedPlatform(t *testing.T) {
	if code, ok := RunPayloadExitCode(errors.New("container runtime requires Linux")); ok || code != 0 {
		t.Fatalf("code=%d ok=%v, want 0,false", code, ok)
	}
}
