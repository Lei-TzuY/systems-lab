//go:build linux

package dns

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"golang.org/x/sys/unix"
)

func TestReadDNSRegistryFileReadsVerifiedRegularFile(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "bridge.json")
	want := []byte(`{"schema_version":1}`)
	if err := os.WriteFile(path, want, 0o600); err != nil {
		t.Fatal(err)
	}

	got, exists, err := readDNSRegistryFile(path, "bridge")
	if err != nil {
		t.Fatalf("read regular registry: %v", err)
	}
	if !exists {
		t.Fatal("regular registry reported missing")
	}
	if string(got) != string(want) {
		t.Fatalf("content mismatch: got %q want %q", got, want)
	}
}

func TestReadDNSRegistryFileRejectsSymlink(t *testing.T) {
	dir := t.TempDir()
	target := filepath.Join(dir, "target.json")
	if err := os.WriteFile(target, []byte(`{"schema_version":1}`), 0o600); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(dir, "bridge.json")
	if err := os.Symlink(target, path); err != nil {
		t.Fatal(err)
	}

	if _, _, err := readDNSRegistryFile(path, "bridge"); err == nil {
		t.Fatal("symlink registry unexpectedly accepted")
	}
}

func TestReadDNSRegistryFileRejectsHardLinkedInode(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "bridge.json")
	if err := os.WriteFile(path, []byte(`{"schema_version":1}`), 0o600); err != nil {
		t.Fatal(err)
	}
	alias := filepath.Join(dir, "alias.json")
	if err := os.Link(path, alias); err != nil {
		t.Fatal(err)
	}

	if _, _, err := readDNSRegistryFile(path, "bridge"); err == nil {
		t.Fatal("hard-linked registry unexpectedly accepted")
	} else if !strings.Contains(err.Error(), "single-linked regular file") {
		t.Fatalf("unexpected hard-link error: %v", err)
	}
}

func TestReadDNSRegistryFileRejectsFIFOWithoutBlocking(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "bridge.json")
	if err := unix.Mkfifo(path, 0o600); err != nil {
		t.Fatal(err)
	}

	done := make(chan error, 1)
	go func() {
		_, _, err := readDNSRegistryFile(path, "bridge")
		done <- err
	}()

	select {
	case err := <-done:
		if err == nil {
			t.Fatal("FIFO registry unexpectedly accepted")
		}
		if !strings.Contains(err.Error(), "regular file") {
			t.Fatalf("unexpected FIFO error: %v", err)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("FIFO registry read blocked; O_NONBLOCK protection is ineffective")
	}
}

func TestReadDNSRegistryFileMissingIsNotAnError(t *testing.T) {
	path := filepath.Join(t.TempDir(), "missing.json")
	data, exists, err := readDNSRegistryFile(path, "missing")
	if err != nil {
		t.Fatalf("missing registry returned error: %v", err)
	}
	if exists || data != nil {
		t.Fatalf("missing registry = (%q, %v), want (nil, false)", data, exists)
	}
}
