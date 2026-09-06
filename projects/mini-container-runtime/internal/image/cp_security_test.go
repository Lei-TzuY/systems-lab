package image

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestParseCopyTargetContainsContainerTraversal(t *testing.T) {
	root := t.TempDir()
	cases := []struct {
		in       string
		wantPath string
	}{
		{"ctr:../../outside", "/outside"},
		{"ctr:/etc/../../secret", "/secret"},
		{`ctr:..\..\windows-secret`, "/windows-secret"},
		{"ctr:relative/file", "/relative/file"},
	}
	for _, tc := range cases {
		t.Run(tc.in, func(t *testing.T) {
			id, containerPath := ParseCopyTarget(tc.in)
			if id != "ctr" {
				t.Fatalf("id=%q, want ctr", id)
			}
			if containerPath != tc.wantPath {
				t.Fatalf("path=%q, want %q", containerPath, tc.wantPath)
			}
			real := filepath.Join(root, strings.TrimPrefix(containerPath, "/"))
			rel, err := filepath.Rel(root, real)
			if err != nil {
				t.Fatal(err)
			}
			if rel == ".." || strings.HasPrefix(rel, ".."+string(os.PathSeparator)) {
				t.Fatalf("canonical container path escaped root: %q -> %q", containerPath, real)
			}
		})
	}
}

func TestParseCopyTargetKeepsWindowsDriveHostPath(t *testing.T) {
	input := `C:\host\file.txt`
	id, path := ParseCopyTarget(input)
	if id != "" || path != input {
		t.Fatalf("ParseCopyTarget(%q)=(%q,%q), want host path unchanged", input, id, path)
	}
}

func TestCopyPathRejectsIntermediateSourceSymlink(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	secret := filepath.Join(outside, "secret.txt")
	if err := os.WriteFile(secret, []byte("outside-secret"), 0o600); err != nil {
		t.Fatal(err)
	}
	inside := filepath.Join(root, "inside")
	if err := os.MkdirAll(inside, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, filepath.Join(inside, "escape")); err != nil {
		t.Fatal(err)
	}
	dst := filepath.Join(t.TempDir(), "copied.txt")
	if err := CopyPath(filepath.Join(inside, "escape", "secret.txt"), dst); err == nil {
		t.Fatal("CopyPath followed intermediate source symlink")
	}
	if _, err := os.Lstat(dst); !os.IsNotExist(err) {
		t.Fatalf("destination created after source escape: %v", err)
	}
}

func TestCopyPathRejectsIntermediateDestinationSymlink(t *testing.T) {
	src := filepath.Join(t.TempDir(), "src.txt")
	if err := os.WriteFile(src, []byte("container-data"), 0o600); err != nil {
		t.Fatal(err)
	}
	dstRoot := t.TempDir()
	outside := t.TempDir()
	if err := os.Symlink(outside, filepath.Join(dstRoot, "escape")); err != nil {
		t.Fatal(err)
	}
	outsideFile := filepath.Join(outside, "victim.txt")
	if err := CopyPath(src, filepath.Join(dstRoot, "escape", "victim.txt")); err == nil {
		t.Fatal("CopyPath followed intermediate destination symlink")
	}
	if _, err := os.Lstat(outsideFile); !os.IsNotExist(err) {
		t.Fatalf("outside file created through destination symlink: %v", err)
	}
}

func TestCopyPathRejectsFinalDestinationSymlink(t *testing.T) {
	src := filepath.Join(t.TempDir(), "src.txt")
	if err := os.WriteFile(src, []byte("new-data"), 0o600); err != nil {
		t.Fatal(err)
	}
	outside := filepath.Join(t.TempDir(), "outside.txt")
	const original = "keep-outside"
	if err := os.WriteFile(outside, []byte(original), 0o600); err != nil {
		t.Fatal(err)
	}
	dst := filepath.Join(t.TempDir(), "dst.txt")
	if err := os.Symlink(outside, dst); err != nil {
		t.Fatal(err)
	}
	if err := CopyPath(src, dst); err == nil {
		t.Fatal("CopyPath accepted symlink destination")
	}
	data, err := os.ReadFile(outside)
	if err != nil || string(data) != original {
		t.Fatalf("outside target changed: data=%q err=%v", data, err)
	}
}

func TestCopyPathCopiesSourceSymlinkWithoutDereference(t *testing.T) {
	outside := filepath.Join(t.TempDir(), "outside.txt")
	if err := os.WriteFile(outside, []byte("do-not-inline"), 0o600); err != nil {
		t.Fatal(err)
	}
	srcDir := t.TempDir()
	src := filepath.Join(srcDir, "link")
	if err := os.Symlink(outside, src); err != nil {
		t.Fatal(err)
	}
	dst := filepath.Join(t.TempDir(), "copied-link")
	if err := CopyPath(src, dst); err != nil {
		t.Fatalf("CopyPath symlink: %v", err)
	}
	info, err := os.Lstat(dst)
	if err != nil {
		t.Fatal(err)
	}
	if info.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("destination is not a symlink: mode=%v", info.Mode())
	}
	target, err := os.Readlink(dst)
	if err != nil {
		t.Fatal(err)
	}
	if target != outside {
		t.Fatalf("symlink target=%q, want %q", target, outside)
	}
}

func TestCopyPathDirectoryPreservesSymlinkAsLink(t *testing.T) {
	src := t.TempDir()
	outside := filepath.Join(t.TempDir(), "outside.txt")
	if err := os.WriteFile(outside, []byte("outside"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(src, "normal.txt"), []byte("normal"), 0o640); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, filepath.Join(src, "link")); err != nil {
		t.Fatal(err)
	}
	dst := filepath.Join(t.TempDir(), "dst")
	if err := CopyPath(src, dst); err != nil {
		t.Fatalf("CopyPath directory: %v", err)
	}
	if data, err := os.ReadFile(filepath.Join(dst, "normal.txt")); err != nil || string(data) != "normal" {
		t.Fatalf("normal file copy: data=%q err=%v", data, err)
	}
	info, err := os.Lstat(filepath.Join(dst, "link"))
	if err != nil || info.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("tree symlink not preserved: info=%v err=%v", info, err)
	}
}
