//go:build linux

package container

import (
	"errors"
	"fmt"
	"os/exec"
	"testing"
)

func runClassifierPayloadExitError(t *testing.T, code int) error {
	t.Helper()
	cmd := exec.Command("sh", "-c", fmt.Sprintf("exit %d", code))
	err := cmd.Run()
	if err == nil {
		t.Fatalf("exit %d unexpectedly succeeded", code)
	}
	var exitErr *exec.ExitError
	if !errors.As(err, &exitErr) {
		t.Fatalf("error=%T %v, want *exec.ExitError", err, err)
	}
	return err
}

func TestRunPayloadExitCodeReturnsPurePayloadStatus(t *testing.T) {
	err := runClassifierPayloadExitError(t, 17)
	code, ok := RunPayloadExitCode(err)
	if !ok || code != 17 {
		t.Fatalf("code=%d ok=%v, want 17,true", code, ok)
	}

	wrapped := fmt.Errorf("payload failed: %w", err)
	code, ok = RunPayloadExitCode(wrapped)
	if !ok || code != 17 {
		t.Fatalf("wrapped code=%d ok=%v, want 17,true", code, ok)
	}
}

func TestRunPayloadExitCodeRejectsRuntimeControlJoin(t *testing.T) {
	payloadErr := runClassifierPayloadExitError(t, 23)
	runtimeErr := &runtimeSetupError{err: errors.New("cgroup cleanup failed")}

	code, ok := RunPayloadExitCode(errors.Join(payloadErr, runtimeErr))
	if ok || code != 0 {
		t.Fatalf("code=%d ok=%v, want runtime failure", code, ok)
	}
}

func TestRunPayloadExitCodeRejectsUnclassifiedJoinedFailure(t *testing.T) {
	payloadErr := runClassifierPayloadExitError(t, 29)
	stateErr := errors.New("state settlement failed")

	code, ok := RunPayloadExitCode(errors.Join(payloadErr, stateErr))
	if ok || code != 0 {
		t.Fatalf("code=%d ok=%v, want generic failure", code, ok)
	}
}

func TestRunPayloadExitCodeAcceptsSinglePayloadThroughErrorsJoin(t *testing.T) {
	payloadErr := runClassifierPayloadExitError(t, 31)
	code, ok := RunPayloadExitCode(errors.Join(payloadErr, nil))
	if !ok || code != 31 {
		t.Fatalf("code=%d ok=%v, want 31,true", code, ok)
	}
}

func TestRunPayloadExitCodeRejectsMultiplePayloadResults(t *testing.T) {
	first := runClassifierPayloadExitError(t, 37)
	secondSame := runClassifierPayloadExitError(t, 37)
	if code, ok := RunPayloadExitCode(errors.Join(first, secondSame)); ok || code != 0 {
		t.Fatalf("same-code multi-exit: code=%d ok=%v, want generic failure", code, ok)
	}

	secondDifferent := runClassifierPayloadExitError(t, 41)
	if code, ok := RunPayloadExitCode(errors.Join(first, secondDifferent)); ok || code != 0 {
		t.Fatalf("different-code multi-exit: code=%d ok=%v, want generic failure", code, ok)
	}
}

func TestRunPayloadExitCodeRejectsSignalExit(t *testing.T) {
	err := exec.Command("sh", "-c", "kill -TERM $$").Run()
	if err == nil {
		t.Fatal("signaled command unexpectedly succeeded")
	}
	code, ok := RunPayloadExitCode(err)
	if ok || code != 0 {
		t.Fatalf("code=%d ok=%v, want generic failure for signal", code, ok)
	}
}

func TestRunPayloadExitCodeRejectsNilAndOrdinaryErrors(t *testing.T) {
	if code, ok := RunPayloadExitCode(nil); ok || code != 0 {
		t.Fatalf("nil: code=%d ok=%v", code, ok)
	}
	if code, ok := RunPayloadExitCode(errors.New("setup failed")); ok || code != 0 {
		t.Fatalf("ordinary: code=%d ok=%v", code, ok)
	}
}
