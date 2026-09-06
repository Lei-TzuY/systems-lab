//go:build linux

package container

import (
	"errors"
	"io"
	"os"
	"strings"
	"testing"
)

func TestRuntimeSyncRequiresExplicitReadyByte(t *testing.T) {
	readPipe, writePipe, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	if err := writePipe.Close(); err != nil {
		t.Fatal(err)
	}

	err = awaitParentReady(readPipe)
	if err == nil {
		t.Fatal("EOF without a ready byte released the child")
	}
	if !errors.Is(err, io.EOF) {
		t.Fatalf("EOF readiness error=%v, want io.EOF", err)
	}
}

func TestRuntimeSyncAcceptsOnlyExpectedReadyByte(t *testing.T) {
	readPipe, writePipe, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := writePipe.Write([]byte{runtimeReadyByte ^ 0xff}); err != nil {
		t.Fatal(err)
	}
	if err := writePipe.Close(); err != nil {
		t.Fatal(err)
	}

	err = awaitParentReady(readPipe)
	if err == nil || !strings.Contains(err.Error(), "invalid runtime ready byte") {
		t.Fatalf("unexpected-byte readiness error=%v", err)
	}
}

func TestReleaseBlockedChildDeliversReadyByte(t *testing.T) {
	readPipe, writePipe, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}

	if err := releaseBlockedChild(writePipe); err != nil {
		t.Fatalf("release child: %v", err)
	}
	if err := awaitParentReady(readPipe); err != nil {
		t.Fatalf("await readiness: %v", err)
	}
}

func TestRuntimeSyncRejectsMissingPipes(t *testing.T) {
	if err := releaseBlockedChild(nil); err == nil {
		t.Fatal("nil writer was accepted")
	}
	if err := awaitParentReady(nil); err == nil {
		t.Fatal("nil reader was accepted")
	}
}

func TestReleaseBlockedChildReportsUndeliveredSignal(t *testing.T) {
	readPipe, writePipe, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	defer readPipe.Close()
	if err := writePipe.Close(); err != nil {
		t.Fatal(err)
	}

	if err := releaseBlockedChild(writePipe); err == nil {
		t.Fatal("write to a closed sync writer reported success")
	}
}
