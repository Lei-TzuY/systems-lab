//go:build linux

package container

import (
	"errors"
	"os"
	"os/exec"
	"strings"
	"testing"
)

func TestAbortPreRunningChildFailureRequiresReapProof(t *testing.T) {
	cause := &runtimeStateError{err: errors.New("persist running state")}
	abortErr := errors.New("wait interrupted")
	called := 0

	err := abortPreRunningChildFailureWithAbort(nil, nil, cause, func(*exec.Cmd, *os.File) (bool, error) {
		called++
		return false, abortErr
	})
	if called != 1 {
		t.Fatalf("abort calls = %d, want 1", called)
	}
	if !errors.Is(err, abortErr) {
		t.Fatalf("error %v does not preserve abort cause", err)
	}
	if !isRuntimeControlError(err) {
		t.Fatalf("error %v must remain a runtime-control failure", err)
	}
	if !strings.Contains(err.Error(), "not confirmed reaped") {
		t.Fatalf("error %q does not report missing reap proof", err)
	}
}

func TestAbortPreRunningChildFailureReapedPreservesOriginalCause(t *testing.T) {
	original := errors.New("capture identity")
	cause := &runtimeStateError{err: original}

	err := abortPreRunningChildFailureWithAbort(nil, nil, cause, func(*exec.Cmd, *os.File) (bool, error) {
		return true, nil
	})
	if !errors.Is(err, original) {
		t.Fatalf("error %v does not preserve original failure", err)
	}
	if !isRuntimeControlError(err) {
		t.Fatalf("error %v must remain a runtime-control failure", err)
	}
	if strings.Contains(err.Error(), "not confirmed reaped") {
		t.Fatalf("reaped child unexpectedly reported missing reap proof: %v", err)
	}
}

func TestAbortPreRunningChildFailureRejectsNilAbort(t *testing.T) {
	original := errors.New("derive cgroup identity")
	err := abortPreRunningChildFailureWithAbort(nil, nil, &runtimeStateError{err: original}, nil)
	if !errors.Is(err, original) {
		t.Fatalf("error %v does not preserve original failure", err)
	}
	if !isRuntimeControlError(err) {
		t.Fatalf("error %v must be a runtime-control failure", err)
	}
	if !strings.Contains(err.Error(), "abort operation is nil") {
		t.Fatalf("error %q does not fail closed on nil abort", err)
	}
}
