//go:build linux

package state

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"golang.org/x/sys/unix"
)

func TestStateStorageUsesPrivateModesAndNoTempArtifacts(t *testing.T) {
	root := filepath.Join(t.TempDir(), "state")
	if err := os.Mkdir(root, 0o755); err != nil {
		t.Fatal(err)
	}
	store, err := Open(root)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}

	for _, dir := range []string{root, filepath.Join(root, "containers"), filepath.Join(root, "images")} {
		info, err := os.Stat(dir)
		if err != nil {
			t.Fatal(err)
		}
		if got := info.Mode().Perm(); got != 0o700 {
			t.Fatalf("%s mode=%#o, want 0700", dir, got)
		}
	}

	ctr := &Container{ID: "private-ctr", Status: StatusStopped, CreatedAt: time.Now()}
	if err := store.Save(ctr); err != nil {
		t.Fatalf("Save: %v", err)
	}
	img := &Image{Name: "private:image", RootFS: "/tmp/rootfs", LoadedAt: time.Now()}
	if err := store.SaveImage(img); err != nil {
		t.Fatalf("SaveImage: %v", err)
	}

	for _, path := range []string{
		filepath.Join(root, "containers", "private-ctr.json"),
		filepath.Join(root, "images", imageMetadataFilename("private:image")),
		filepath.Join(root, ".state.lock"),
	} {
		info, err := os.Stat(path)
		if err != nil {
			t.Fatal(err)
		}
		if got := info.Mode().Perm(); got != 0o600 {
			t.Fatalf("%s mode=%#o, want 0600", path, got)
		}
	}

	for _, dir := range []string{filepath.Join(root, "containers"), filepath.Join(root, "images")} {
		entries, err := os.ReadDir(dir)
		if err != nil {
			t.Fatal(err)
		}
		for _, entry := range entries {
			if strings.HasPrefix(entry.Name(), ".tmp-") {
				t.Fatalf("temporary state artifact left behind: %s", filepath.Join(dir, entry.Name()))
			}
		}
	}
}

func TestOpenRejectsSymlinkStateDirectory(t *testing.T) {
	parent := t.TempDir()
	realDir := filepath.Join(parent, "real")
	if err := os.Mkdir(realDir, 0o700); err != nil {
		t.Fatal(err)
	}
	linkDir := filepath.Join(parent, "link")
	if err := os.Symlink(realDir, linkDir); err != nil {
		t.Fatal(err)
	}
	if _, err := Open(linkDir); err == nil || !strings.Contains(err.Error(), "real directory") {
		t.Fatalf("symlink state directory error=%v", err)
	}
}

func TestContainerStateRejectsSymlinkAndFIFO(t *testing.T) {
	root := t.TempDir()
	store, err := Open(root)
	if err != nil {
		t.Fatal(err)
	}
	containers := filepath.Join(root, "containers")

	target := filepath.Join(root, "target.json")
	if err := os.WriteFile(target, []byte(`{"id":"symlinked","status":"stopped"}`), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(target, filepath.Join(containers, "symlinked.json")); err != nil {
		t.Fatal(err)
	}
	if _, err := store.Get("symlinked"); err == nil {
		t.Fatal("symlinked container state unexpectedly accepted")
	}

	fifo := filepath.Join(containers, "fifo-state.json")
	if err := unix.Mkfifo(fifo, 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := store.Get("fifo-state"); err == nil || !strings.Contains(err.Error(), "regular file") {
		t.Fatalf("FIFO container state error=%v", err)
	}
}

func TestImageStateRejectsSymlinkAndCorruption(t *testing.T) {
	root := t.TempDir()
	store, err := Open(root)
	if err != nil {
		t.Fatal(err)
	}
	images := filepath.Join(root, "images")

	target := filepath.Join(root, "image-target.json")
	if err := os.WriteFile(target, []byte(`{"name":"linked","rootfs":"/tmp/root"}`), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(target, filepath.Join(images, "linked.json")); err != nil {
		t.Fatal(err)
	}
	if _, err := store.ListImages(); err == nil {
		t.Fatal("symlinked image state unexpectedly accepted")
	}
	if err := os.Remove(filepath.Join(images, "linked.json")); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(images, "broken.json"), []byte(`{"name":`), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := store.ListImages(); err == nil || !strings.Contains(err.Error(), "unmarshal image state") {
		t.Fatalf("corrupt image state error=%v", err)
	}
}

func TestListContainersFailsClosedOnCorruptState(t *testing.T) {
	root := t.TempDir()
	store, err := Open(root)
	if err != nil {
		t.Fatal(err)
	}
	if err := store.Save(&Container{ID: "good", Status: StatusStopped}); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "containers", "broken.json"), []byte(`{"id":`), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := store.List(); err == nil || !strings.Contains(err.Error(), "load container state") {
		t.Fatalf("corrupt container list error=%v", err)
	}
}

func TestSaveImageRejectsNilAndEmptyIdentity(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := store.SaveImage(nil); err == nil || !strings.Contains(err.Error(), "nil") {
		t.Fatalf("nil image error=%v", err)
	}
	if err := store.SaveImage(&Image{}); err == nil || !strings.Contains(err.Error(), "name or ID") {
		t.Fatalf("empty image identity error=%v", err)
	}
}

func TestDurableDeleteRetrySyncsAlreadyAbsentPath(t *testing.T) {
	dir := t.TempDir()
	missing := filepath.Join(dir, "already-gone.json")
	if err := removeStateFileDurable(dir, missing, "test state"); err != nil {
		t.Fatalf("durable delete of already-absent path: %v", err)
	}
}
