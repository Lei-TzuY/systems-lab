//go:build linux

package builder

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"golang.org/x/sys/unix"
)

func TestPinnedBuildSourceRejectsLeafSymlinkReplacementAfterContextValidation(t *testing.T) {
	base := t.TempDir()
	contextDir := filepath.Join(base, "context")
	if err := os.Mkdir(contextDir, 0o700); err != nil {
		t.Fatal(err)
	}
	source := filepath.Join(contextDir, "source.txt")
	if err := os.WriteFile(source, []byte("safe"), 0o600); err != nil {
		t.Fatal(err)
	}
	outside := filepath.Join(base, "secret.txt")
	if err := os.WriteFile(outside, []byte("secret"), 0o600); err != nil {
		t.Fatal(err)
	}

	resolved, err := resolveBuildContextSource(contextDir, "source.txt")
	if err != nil {
		t.Fatalf("context validation unexpectedly failed: %v", err)
	}
	var swapped bool
	in, err := openPinnedBuildSource(resolved, func(_ int, _ string, final bool) error {
		if !final || swapped {
			return nil
		}
		swapped = true
		if err := os.Rename(source, source+".original"); err != nil {
			return err
		}
		return os.Symlink(outside, source)
	})
	if in != nil {
		_ = in.Close()
	}
	if err == nil || !strings.Contains(err.Error(), "must not be a symlink") {
		t.Fatalf("leaf replacement error=%v, want symlink rejection", err)
	}
	if !swapped {
		t.Fatal("test hook did not replace source leaf")
	}
}

func TestPinnedBuildSourceRejectsParentSymlinkReplacement(t *testing.T) {
	base := t.TempDir()
	contextDir := filepath.Join(base, "context")
	sourceDir := filepath.Join(contextDir, "src")
	if err := os.MkdirAll(sourceDir, 0o700); err != nil {
		t.Fatal(err)
	}
	source := filepath.Join(sourceDir, "file.txt")
	if err := os.WriteFile(source, []byte("safe"), 0o600); err != nil {
		t.Fatal(err)
	}
	outsideDir := filepath.Join(base, "outside")
	if err := os.Mkdir(outsideDir, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(outsideDir, "file.txt"), []byte("secret"), 0o600); err != nil {
		t.Fatal(err)
	}

	resolved, err := resolveBuildContextSource(contextDir, "src/file.txt")
	if err != nil {
		t.Fatalf("context validation unexpectedly failed: %v", err)
	}
	var swapped bool
	in, err := openPinnedBuildSource(resolved, func(_ int, name string, final bool) error {
		if final || swapped || name != "src" {
			return nil
		}
		swapped = true
		if err := os.Rename(sourceDir, sourceDir+".original"); err != nil {
			return err
		}
		return os.Symlink(outsideDir, sourceDir)
	})
	if in != nil {
		_ = in.Close()
	}
	if err == nil || !strings.Contains(err.Error(), "without following symlinks") {
		t.Fatalf("parent replacement error=%v, want nofollow rejection", err)
	}
	if !swapped {
		t.Fatal("test hook did not replace source parent")
	}
}

func TestPinnedBuildDirectorySurvivesSourcePathReplacement(t *testing.T) {
	base := t.TempDir()
	source := filepath.Join(base, "source")
	if err := os.Mkdir(source, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(source, "original.txt"), []byte("original"), 0o600); err != nil {
		t.Fatal(err)
	}

	in, err := openPinnedBuildSource(source, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer in.Close()

	pinnedPath := source + ".pinned"
	if err := os.Rename(source, pinnedPath); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(source, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(source, "replacement.txt"), []byte("replacement"), 0o600); err != nil {
		t.Fatal(err)
	}

	dst := filepath.Join(base, "dst")
	if err := os.Mkdir(dst, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := copyPinnedBuildNode(in, source, dst, "/", true); err != nil {
		t.Fatalf("copy pinned source: %v", err)
	}
	data, err := os.ReadFile(filepath.Join(dst, "original.txt"))
	if err != nil || string(data) != "original" {
		t.Fatalf("pinned source data=%q err=%v", data, err)
	}
	if _, err := os.Stat(filepath.Join(dst, "replacement.txt")); !os.IsNotExist(err) {
		t.Fatalf("copy followed replacement source generation: %v", err)
	}
}

func TestPinObservedBuildChildRejectsGenerationReplacement(t *testing.T) {
	base := t.TempDir()
	source := filepath.Join(base, "source")
	if err := os.Mkdir(source, 0o700); err != nil {
		t.Fatal(err)
	}
	child := filepath.Join(source, "child.txt")
	if err := os.WriteFile(child, []byte("first"), 0o600); err != nil {
		t.Fatal(err)
	}

	dir, err := openPinnedBuildSource(source, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer dir.Close()
	var observed unix.Stat_t
	if err := unix.Fstatat(int(dir.Fd()), "child.txt", &observed, unix.AT_SYMLINK_NOFOLLOW); err != nil {
		t.Fatal(err)
	}
	if err := os.Rename(child, child+".old"); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(child, []byte("second"), 0o600); err != nil {
		t.Fatal(err)
	}

	fd, err := pinObservedBuildChild(int(dir.Fd()), "child.txt", &observed)
	if fd >= 0 {
		_ = unix.Close(fd)
	}
	if err == nil || !strings.Contains(err.Error(), "changed generation") {
		t.Fatalf("generation replacement error=%v", err)
	}
}

func TestPinnedBuildTreeCopiesSymlinkWithoutFollowingSourceTarget(t *testing.T) {
	base := t.TempDir()
	source := filepath.Join(base, "source")
	if err := os.Mkdir(source, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(source, "file.txt"), []byte("payload"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("file.txt", filepath.Join(source, "link")); err != nil {
		t.Fatal(err)
	}
	dst := filepath.Join(base, "dst")
	if err := os.Mkdir(dst, 0o700); err != nil {
		t.Fatal(err)
	}

	if err := copyTree(source, dst, "/", true); err != nil {
		t.Fatalf("copy pinned tree with symlink: %v", err)
	}
	target, err := os.Readlink(filepath.Join(dst, "link"))
	if err != nil {
		t.Fatal(err)
	}
	if target != "file.txt" {
		t.Fatalf("copied symlink target=%q, want file.txt", target)
	}
}
