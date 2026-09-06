//go:build linux

package container

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestValidateAdmittedRootFSIdentityRejectsReplacement(t *testing.T) {
	parent := t.TempDir()
	root := filepath.Join(parent, "rootfs")
	if err := os.Mkdir(root, 0o755); err != nil {
		t.Fatal(err)
	}
	admitted, err := os.Stat(root)
	if err != nil {
		t.Fatal(err)
	}
	old := filepath.Join(parent, "old-rootfs")
	if err := os.Rename(root, old); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(root, 0o755); err != nil {
		t.Fatal(err)
	}

	err = validateAdmittedRootFSIdentity(Config{RootFS: root, RootFSIdentity: admitted})
	if err == nil {
		t.Fatal("expected rootfs identity drift rejection")
	}
	if !strings.Contains(err.Error(), "filesystem identity changed") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestValidateAdmittedRootFSIdentityAcceptsStableDirectory(t *testing.T) {
	root := t.TempDir()
	admitted, err := os.Stat(root)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateAdmittedRootFSIdentity(Config{RootFS: root, RootFSIdentity: admitted}); err != nil {
		t.Fatalf("stable rootfs rejected: %v", err)
	}
}

func TestValidateAdmittedRootFSIdentityRejectsMissingPath(t *testing.T) {
	root := t.TempDir()
	admitted, err := os.Stat(root)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(root); err != nil {
		t.Fatal(err)
	}
	if err := validateAdmittedRootFSIdentity(Config{RootFS: root, RootFSIdentity: admitted}); err == nil {
		t.Fatal("expected missing rootfs rejection")
	}
}

func TestValidateAdmittedRootFSIdentityPreservesUnmanagedCompatibility(t *testing.T) {
	if err := validateAdmittedRootFSIdentity(Config{RootFS: "/path/need/not/exist"}); err != nil {
		t.Fatalf("unmanaged config without admission identity rejected: %v", err)
	}
}
