package network

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestRunTrustedHostToolIgnoresPATH(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("executable shell fixture requires POSIX")
	}

	trustedDir := t.TempDir()
	attackerDir := t.TempDir()
	writeExecutable := func(dir, name, body string) {
		t.Helper()
		path := filepath.Join(dir, name)
		if err := os.WriteFile(path, []byte("#!/bin/sh\nprintf '%s' '"+body+"'\n"), 0o755); err != nil {
			t.Fatal(err)
		}
	}
	writeExecutable(trustedDir, "ip", "trusted")
	writeExecutable(attackerDir, "ip", "attacker")

	oldDirs := trustedHostToolDirs
	trustedHostToolDirs = []string{trustedDir}
	t.Cleanup(func() { trustedHostToolDirs = oldDirs })
	t.Setenv("PATH", attackerDir)

	out, err := runTrustedHostTool("ip", "ignored")
	if err != nil {
		t.Fatalf("run trusted tool: %v", err)
	}
	if got := strings.TrimSpace(string(out)); got != "trusted" {
		t.Fatalf("ran unexpected executable: got %q, want trusted", got)
	}
}

func TestResolveTrustedHostToolRejectsSymlink(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlink semantics differ on Windows")
	}

	dir := t.TempDir()
	target := filepath.Join(dir, "real-ip")
	if err := os.WriteFile(target, []byte("#!/bin/sh\nexit 0\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	link := filepath.Join(dir, "ip")
	if err := os.Symlink(target, link); err != nil {
		t.Skipf("symlink unsupported: %v", err)
	}

	if got, err := resolveTrustedHostTool("ip", []string{dir}); err == nil {
		t.Fatalf("resolved unsafe symlink %q", got)
	}
}

func TestResolveTrustedHostToolRejectsInvalidName(t *testing.T) {
	if got, err := resolveTrustedHostTool("../ip", []string{t.TempDir()}); err == nil {
		t.Fatalf("resolved invalid executable name %q", got)
	}
}
