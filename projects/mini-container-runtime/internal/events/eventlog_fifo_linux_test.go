//go:build linux

package events

import (
	"path/filepath"
	"testing"
	"time"

	"golang.org/x/sys/unix"
)

func TestEventLogRejectsFIFOWithoutBlocking(t *testing.T) {
	path := filepath.Join(t.TempDir(), "events.log")
	if err := unix.Mkfifo(path, 0o600); err != nil {
		t.Fatal(err)
	}

	done := make(chan error, 1)
	go func() {
		f, err := openEventLogForRead(path)
		if f != nil {
			_ = f.Close()
		}
		done <- err
	}()

	select {
	case err := <-done:
		if err == nil {
			t.Fatal("openEventLogForRead accepted FIFO")
		}
	case <-time.After(time.Second):
		t.Fatal("openEventLogForRead blocked on FIFO")
	}
}
