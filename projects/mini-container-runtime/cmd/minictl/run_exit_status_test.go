package main

import (
	"errors"
	"fmt"
	"os/exec"
	"testing"
)

func cliPayloadExitError(t *testing.T, code int) error {
	t.Helper()
	err := exec.Command("sh", "-c", fmt.Sprintf("exit %d", code)).Run()
	if err == nil {
		t.Fatalf("exit %d unexpectedly succeeded", code)
	}
	return err
}

func TestRunCommandExitCodePropagatesPurePayloadStatus(t *testing.T) {
	if got := runCommandExitCode(cliPayloadExitError(t, 17)); got != 17 {
		t.Fatalf("exit code=%d, want 17", got)
	}
}

func TestRunCommandExitCodeUsesGenericStatusForNonPayloadFailure(t *testing.T) {
	if got := runCommandExitCode(errors.New("runtime failure")); got != 1 {
		t.Fatalf("exit code=%d, want 1", got)
	}
}
