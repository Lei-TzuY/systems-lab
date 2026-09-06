//go:build linux

package container

import (
	"errors"
	"os"
	"os/exec"
	"testing"
)

func TestAbortBlockedChildWithOpsConfirmsSuccessfulWait(t *testing.T) {
	var closed, killed, waited int
	reaped, err := abortBlockedChildWithOps(
		func() error { closed++; return nil },
		func() error { killed++; return nil },
		func() error { waited++; return nil },
	)
	if err != nil {
		t.Fatalf("abort error: %v", err)
	}
	if !reaped {
		t.Fatal("successful Wait did not confirm reaped child")
	}
	if closed != 1 || killed != 1 || waited != 1 {
		t.Fatalf("calls close=%d kill=%d wait=%d, want 1/1/1", closed, killed, waited)
	}
}

func TestAbortBlockedChildWithOpsExitErrorStillConfirmsReaped(t *testing.T) {
	reaped, err := abortBlockedChildWithOps(nil, func() error { return nil }, func() error {
		return &exec.ExitError{}
	})
	if err != nil {
		t.Fatalf("abort error: %v", err)
	}
	if !reaped {
		t.Fatal("ExitError from Wait must still prove the child was reaped")
	}
}

func TestAbortBlockedChildWithOpsProcessDoneStillWaits(t *testing.T) {
	waited := 0
	reaped, err := abortBlockedChildWithOps(nil, func() error { return os.ErrProcessDone }, func() error {
		waited++
		return nil
	})
	if err != nil {
		t.Fatalf("abort error: %v", err)
	}
	if !reaped || waited != 1 {
		t.Fatalf("reaped=%v waited=%d, want true/1", reaped, waited)
	}
}

func TestAbortBlockedChildWithOpsKillFailureDoesNotWait(t *testing.T) {
	killErr := errors.New("kill denied")
	waited := 0
	reaped, err := abortBlockedChildWithOps(nil, func() error { return killErr }, func() error {
		waited++
		return nil
	})
	if reaped {
		t.Fatal("kill failure incorrectly confirmed reaped child")
	}
	if !errors.Is(err, killErr) {
		t.Fatalf("error=%v, want kill cause", err)
	}
	if waited != 0 {
		t.Fatalf("wait called %d times after genuine kill failure", waited)
	}
}

func TestAbortBlockedChildWithOpsWaitFailureDoesNotConfirmReap(t *testing.T) {
	waitErr := errors.New("wait unavailable")
	reaped, err := abortBlockedChildWithOps(nil, func() error { return nil }, func() error { return waitErr })
	if reaped {
		t.Fatal("non-process-status Wait failure incorrectly confirmed reap")
	}
	if !errors.Is(err, waitErr) {
		t.Fatalf("error=%v, want wait cause", err)
	}
}

func TestAbortBlockedChildWithOpsPreservesCloseFailureAfterReap(t *testing.T) {
	closeErr := errors.New("close failed")
	reaped, err := abortBlockedChildWithOps(
		func() error { return closeErr },
		func() error { return nil },
		func() error { return nil },
	)
	if !reaped {
		t.Fatal("successful kill/wait should confirm reap despite readiness close error")
	}
	if !errors.Is(err, closeErr) {
		t.Fatalf("error=%v, want readiness close cause", err)
	}
}

func TestAbortBlockedChildCheckedRejectsMissingProcess(t *testing.T) {
	if reaped, err := abortBlockedChildChecked(nil, nil); reaped || err == nil {
		t.Fatalf("nil cmd result reaped=%v err=%v", reaped, err)
	}
	if reaped, err := abortBlockedChildChecked(&exec.Cmd{}, nil); reaped || err == nil {
		t.Fatalf("missing process result reaped=%v err=%v", reaped, err)
	}
}
