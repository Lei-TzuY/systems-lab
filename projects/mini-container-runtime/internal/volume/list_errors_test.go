package volume

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func prepareVolumeListTestRoot(t *testing.T) string {
	t.Helper()
	home := filepath.Join(t.TempDir(), "home")
	if err := os.MkdirAll(home, 0o700); err != nil {
		t.Fatal(err)
	}
	t.Setenv("HOME", home)
	root := DefaultVolumeDir()
	if err := os.MkdirAll(root, 0o700); err != nil {
		t.Fatal(err)
	}
	return root
}

func TestListVolumesSurfacesBrokenManagedMetadata(t *testing.T) {
	root := prepareVolumeListTestRoot(t)
	if err := os.MkdirAll(filepath.Join(root, "broken", "_data"), 0o755); err != nil {
		t.Fatal(err)
	}

	vols, err := ListVolumes()
	if err == nil {
		t.Fatalf("ListVolumes unexpectedly hid broken managed volume: %+v", vols)
	}
	if !strings.Contains(err.Error(), `read managed volume "broken"`) || !strings.Contains(err.Error(), "volume metadata") {
		t.Fatalf("broken volume list error=%v", err)
	}
}

func TestListVolumesRejectsValidNameNonDirectoryEntry(t *testing.T) {
	root := prepareVolumeListTestRoot(t)
	entry := filepath.Join(root, "notdir")
	if err := os.WriteFile(entry, []byte("not a volume directory"), 0o600); err != nil {
		t.Fatal(err)
	}

	vols, err := ListVolumes()
	if err == nil {
		t.Fatalf("ListVolumes unexpectedly hid non-directory managed entry: %+v", vols)
	}
	if !strings.Contains(err.Error(), `managed volume entry "notdir" is not a directory`) {
		t.Fatalf("non-directory volume list error=%v", err)
	}
}

func TestListVolumesStillIgnoresInvalidNameEntries(t *testing.T) {
	root := prepareVolumeListTestRoot(t)
	if err := os.WriteFile(filepath.Join(root, ".unmanaged"), []byte("ignored"), 0o600); err != nil {
		t.Fatal(err)
	}

	vols, err := ListVolumes()
	if err != nil {
		t.Fatalf("invalid-name unmanaged entry caused list failure: %v", err)
	}
	if len(vols) != 0 {
		t.Fatalf("unexpected managed volumes: %+v", vols)
	}
}
