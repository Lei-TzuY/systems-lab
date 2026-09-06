package daemon

import (
	"context"
	"net"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
	"time"
)

func TestResolveListenAddress(t *testing.T) {
	tests := []struct {
		name        string
		raw         string
		wantNetwork string
		wantAddr    string
		wantErr     bool
	}{
		{"default", "", "unix", "/tmp/minictl.sock", false},
		{"unix", "unix:///tmp/test.sock", "unix", "/tmp/test.sock", false},
		{"tcp prefixed", "tcp://127.0.0.1:2375", "tcp", "127.0.0.1:2375", false},
		{"tcp bare", "127.0.0.1:2375", "tcp", "127.0.0.1:2375", false},
		{"empty unix", "unix://", "", "", true},
		{"relative unix", "unix://relative.sock", "", "", true},
		{"empty tcp", "tcp://", "", "", true},
		{"unsupported", "udp://127.0.0.1:1", "", "", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			network, addr, err := resolveListenAddress(tt.raw)
			if tt.wantErr {
				if err == nil {
					t.Fatalf("expected error, got network=%q addr=%q", network, addr)
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if network != tt.wantNetwork || addr != tt.wantAddr {
				t.Fatalf("got (%q,%q), want (%q,%q)", network, addr, tt.wantNetwork, tt.wantAddr)
			}
		})
	}
}

func TestNewServerRefusesExistingNonSocketPath(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("unix sockets are not portable to Windows")
	}

	dir := t.TempDir()
	path := filepath.Join(dir, "daemon.sock")
	const original = "do-not-delete"
	if err := os.WriteFile(path, []byte(original), 0o600); err != nil {
		t.Fatal(err)
	}

	_, err := NewServer(Config{ListenAddr: "unix://" + path, StoreDir: filepath.Join(dir, "state")})
	if err == nil || !strings.Contains(err.Error(), "refusing to remove non-socket") {
		t.Fatalf("expected non-socket refusal, got %v", err)
	}
	data, readErr := os.ReadFile(path)
	if readErr != nil {
		t.Fatalf("existing file was removed or damaged: %v", readErr)
	}
	if string(data) != original {
		t.Fatalf("existing file changed: %q", data)
	}
}

func TestNewServerRefusesExistingSymlink(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("unix socket test")
	}

	dir := t.TempDir()
	target := filepath.Join(dir, "target")
	path := filepath.Join(dir, "daemon.sock")
	if err := os.WriteFile(target, []byte("target-data"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(target, path); err != nil {
		t.Fatal(err)
	}

	_, err := NewServer(Config{ListenAddr: "unix://" + path, StoreDir: filepath.Join(dir, "state")})
	if err == nil {
		t.Fatal("expected symlink path to be refused")
	}
	data, readErr := os.ReadFile(target)
	if readErr != nil || string(data) != "target-data" {
		t.Fatalf("symlink target was damaged: data=%q err=%v", data, readErr)
	}
}

func TestNewServerRefusesExistingSocket(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("unix socket test")
	}

	dir := t.TempDir()
	path := filepath.Join(dir, "daemon.sock")
	listener, err := net.Listen("unix", path)
	if err != nil {
		t.Fatalf("create existing socket: %v", err)
	}
	defer listener.Close()

	_, err = NewServer(Config{ListenAddr: "unix://" + path, StoreDir: filepath.Join(dir, "state")})
	if err == nil || !strings.Contains(err.Error(), "already exists") {
		t.Fatalf("expected existing socket refusal, got %v", err)
	}
	if _, statErr := os.Lstat(path); statErr != nil {
		t.Fatalf("existing socket path was removed: %v", statErr)
	}
}

func TestNewServerUnixSocketPermissionsAndCleanup(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("unix socket test")
	}

	dir := t.TempDir()
	path := filepath.Join(dir, "daemon.sock")
	srv, err := NewServer(Config{ListenAddr: "unix://" + path, StoreDir: filepath.Join(dir, "state")})
	if err != nil {
		t.Fatalf("NewServer: %v", err)
	}

	info, err := os.Lstat(path)
	if err != nil {
		t.Fatalf("stat socket: %v", err)
	}
	if info.Mode()&os.ModeSocket == 0 {
		t.Fatalf("path is not a socket: %s", info.Mode())
	}
	if got := info.Mode().Perm(); got != unixSocketMode {
		t.Fatalf("socket permissions = %o, want %o", got, unixSocketMode)
	}
	if srv.httpServer.ReadHeaderTimeout != 5*time.Second || srv.httpServer.WriteTimeout != 10*time.Second || srv.httpServer.MaxHeaderBytes != 64<<10 {
		t.Fatalf("unexpected HTTP server limits: %+v", srv.httpServer)
	}

	if err := srv.Stop(context.Background()); err != nil {
		t.Fatalf("Stop: %v", err)
	}
	if _, err := os.Lstat(path); !os.IsNotExist(err) {
		t.Fatalf("socket path still exists after Stop: %v", err)
	}
}

func TestUnixListenerCloseDoesNotAutoUnlink(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("unix socket test")
	}

	dir := t.TempDir()
	path := filepath.Join(dir, "daemon.sock")
	srv, err := NewServer(Config{ListenAddr: "unix://" + path, StoreDir: filepath.Join(dir, "state")})
	if err != nil {
		t.Fatalf("NewServer: %v", err)
	}
	if err := srv.listener.Close(); err != nil {
		t.Fatalf("close listener: %v", err)
	}
	if _, err := os.Lstat(path); err != nil {
		t.Fatalf("listener close auto-unlinked socket: %v", err)
	}
	if err := removeUnixSocketIfSame(path, srv.socketInfo); err != nil {
		t.Fatalf("manual safe cleanup: %v", err)
	}
}

func TestRemoveUnixSocketRefusesRegularFile(t *testing.T) {
	path := filepath.Join(t.TempDir(), "keep-me")
	if err := os.WriteFile(path, []byte("keep"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := removeUnixSocketIfSame(path, nil); err == nil {
		t.Fatal("expected regular file cleanup refusal")
	}
	if data, err := os.ReadFile(path); err != nil || string(data) != "keep" {
		t.Fatalf("regular file was removed or changed: data=%q err=%v", data, err)
	}
}

func TestRemoveUnixSocketRefusesDifferentIdentity(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("unix socket test")
	}

	dir := t.TempDir()
	firstPath := filepath.Join(dir, "first.sock")
	secondPath := filepath.Join(dir, "second.sock")

	first, err := net.Listen("unix", firstPath)
	if err != nil {
		t.Fatal(err)
	}
	defer first.Close()
	second, err := net.Listen("unix", secondPath)
	if err != nil {
		t.Fatal(err)
	}
	defer second.Close()

	firstInfo, err := os.Lstat(firstPath)
	if err != nil {
		t.Fatal(err)
	}
	if err := removeUnixSocketIfSame(secondPath, firstInfo); err == nil || !strings.Contains(err.Error(), "identity changed") {
		t.Fatalf("expected identity mismatch refusal, got %v", err)
	}
	if _, err := os.Lstat(secondPath); err != nil {
		t.Fatalf("different socket was removed: %v", err)
	}
}

func TestStartReturnsNilAfterGracefulStop(t *testing.T) {
	srv, err := NewServer(Config{ListenAddr: "tcp://127.0.0.1:0", StoreDir: t.TempDir()})
	if err != nil {
		t.Fatalf("NewServer: %v", err)
	}

	done := make(chan error, 1)
	go func() { done <- srv.Start() }()

	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	if err := srv.Stop(ctx); err != nil {
		t.Fatalf("Stop: %v", err)
	}

	select {
	case err := <-done:
		if err != nil {
			t.Fatalf("Start returned error after graceful stop: %v", err)
		}
	case <-time.After(time.Second):
		t.Fatal("Start did not return after Stop")
	}
}
