//go:build unix

package logs

import (
	"os"
	"path/filepath"
	"testing"

	"golang.org/x/sys/unix"
)

func TestCompressRotatedLogRejectsNonRegularDestinationBeforeWriting(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log.1")
	if err := os.WriteFile(logPath, []byte("payload\n"), 0644); err != nil {
		t.Fatal(err)
	}

	gzPath := logPath + ".gz"
	if err := unix.Mkfifo(gzPath, 0600); err != nil {
		t.Fatalf("mkfifo: %v", err)
	}
	readerFD, err := unix.Open(gzPath, unix.O_RDONLY|unix.O_NONBLOCK, 0)
	if err != nil {
		t.Fatalf("open fifo reader: %v", err)
	}
	defer unix.Close(readerFD)

	if err := CompressRotatedLog(logPath); err == nil {
		t.Fatal("expected non-regular gzip destination to be rejected")
	}

	buf := make([]byte, 64)
	n, err := unix.Read(readerFD, buf)
	if err != nil && err != unix.EAGAIN && err != unix.EWOULDBLOCK {
		t.Fatalf("read fifo: %v", err)
	}
	if n != 0 {
		t.Fatalf("non-regular gzip destination received %d bytes before rejection", n)
	}
	if _, err := os.Stat(logPath); err != nil {
		t.Fatalf("source log should remain after rejecting destination: %v", err)
	}
}
