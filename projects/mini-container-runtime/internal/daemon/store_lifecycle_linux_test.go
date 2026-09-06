package daemon

import (
	"context"
	"os"
	"path/filepath"
	"testing"
)

func openFDCount(t *testing.T) int {
	t.Helper()
	entries, err := os.ReadDir("/proc/self/fd")
	if err != nil {
		t.Fatalf("read /proc/self/fd: %v", err)
	}
	return len(entries)
}

func TestNewServerFailureReleasesStateStoreFDs(t *testing.T) {
	dir := t.TempDir()
	socketPath := filepath.Join(dir, "occupied.sock")
	if err := os.WriteFile(socketPath, []byte("occupied"), 0o600); err != nil {
		t.Fatal(err)
	}
	storeDir := filepath.Join(dir, "state")

	// Warm the state directory creation path before taking the baseline so the
	// assertion measures descriptors owned by failed constructors, not setup.
	srv, err := NewServer(Config{ListenAddr: "tcp://127.0.0.1:0", StoreDir: storeDir})
	if err != nil {
		t.Fatalf("warm NewServer: %v", err)
	}
	if err := srv.Stop(context.Background()); err != nil {
		t.Fatalf("warm Stop: %v", err)
	}
	baseline := openFDCount(t)

	for i := 0; i < 32; i++ {
		if _, err := NewServer(Config{ListenAddr: "unix://" + socketPath, StoreDir: storeDir}); err == nil {
			t.Fatal("expected constructor failure for occupied non-socket path")
		}
	}

	if got := openFDCount(t); got > baseline+2 {
		t.Fatalf("failed NewServer calls leaked file descriptors: baseline=%d after=%d", baseline, got)
	}
}

func TestStopReleasesOwnedStateStoreFDsAndIsIdempotent(t *testing.T) {
	baseline := openFDCount(t)
	srv, err := NewServer(Config{ListenAddr: "tcp://127.0.0.1:0", StoreDir: t.TempDir()})
	if err != nil {
		t.Fatalf("NewServer: %v", err)
	}
	opened := openFDCount(t)
	if opened <= baseline {
		t.Fatalf("NewServer did not acquire observable resources: baseline=%d opened=%d", baseline, opened)
	}

	if err := srv.Stop(context.Background()); err != nil {
		t.Fatalf("Stop: %v", err)
	}
	if err := srv.Stop(context.Background()); err != nil {
		t.Fatalf("second Stop: %v", err)
	}
	if got := openFDCount(t); got > baseline+2 {
		t.Fatalf("Stop leaked server/store file descriptors: baseline=%d after=%d", baseline, got)
	}
}
