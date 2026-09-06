package state

import (
	"path/filepath"
	"strings"
	"testing"
)

func TestDeleteImageIfMatchWithCleanupArmsLastReferenceAndBlocksRepublish(t *testing.T) {
	base := t.TempDir()
	st, err := Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	rootFS := filepath.Join(base, "images", "cleanup-id", "rootfs")
	img := &Image{ID: "cleanup-id", Name: "cleanup:old", Tag: "<none>", RootFS: rootFS}
	if err := st.SaveImage(img); err != nil {
		t.Fatal(err)
	}
	expected, err := st.GetImage(img.Name)
	if err != nil {
		t.Fatal(err)
	}
	cleanup := ImageCleanup{ID: expected.ID, RootFS: expected.RootFS}

	removed, armed, err := st.DeleteImageIfMatchWithCleanup(img.Name, expected, cleanup)
	if err != nil {
		t.Fatalf("DeleteImageIfMatchWithCleanup: %v", err)
	}
	if !armed {
		t.Fatal("last rootfs reference did not arm durable cleanup")
	}
	if removed.ID != expected.ID || removed.RootFS != expected.RootFS {
		t.Fatalf("removed image=%+v, want %+v", removed, expected)
	}
	if _, err := st.GetImage(img.Name); err == nil {
		t.Fatal("metadata still exists after cleanup transaction")
	}
	cleanups, err := st.ListImageCleanups()
	if err != nil {
		t.Fatal(err)
	}
	if len(cleanups) != 1 || cleanups[0] != cleanup {
		t.Fatalf("pending cleanups=%+v, want [%+v]", cleanups, cleanup)
	}

	republished := *expected
	republished.Name = "cleanup:new"
	republished.Tag = "new"
	if err := st.SaveImage(&republished); err == nil || !strings.Contains(err.Error(), "pending cleanup") {
		t.Fatalf("SaveImage during cleanup error=%v, want pending-cleanup rejection", err)
	}

	cleared, err := st.ClearImageCleanupIfMatch(cleanup)
	if err != nil {
		t.Fatal(err)
	}
	if !cleared {
		t.Fatal("cleanup ownership was not cleared")
	}
	if err := st.SaveImage(&republished); err != nil {
		t.Fatalf("SaveImage after cleanup clear: %v", err)
	}
}

func TestDeleteImageIfMatchWithCleanupDoesNotArmWhileAliasReferencesRootFS(t *testing.T) {
	base := t.TempDir()
	st, err := Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	rootFS := filepath.Join(base, "images", "shared-cleanup-id", "rootfs")
	old := &Image{ID: "shared-cleanup-id", Name: "shared:old", Tag: "<none>", RootFS: rootFS}
	live := &Image{ID: "shared-cleanup-id", Name: "shared:latest", Tag: "latest", RootFS: rootFS}
	if err := st.SaveImage(old); err != nil {
		t.Fatal(err)
	}
	if err := st.SaveImage(live); err != nil {
		t.Fatal(err)
	}
	expected, err := st.GetImage(old.Name)
	if err != nil {
		t.Fatal(err)
	}
	cleanup := ImageCleanup{ID: expected.ID, RootFS: expected.RootFS}

	_, armed, err := st.DeleteImageIfMatchWithCleanup(old.Name, expected, cleanup)
	if err != nil {
		t.Fatalf("DeleteImageIfMatchWithCleanup: %v", err)
	}
	if armed {
		t.Fatal("cleanup armed while another alias still referenced rootfs")
	}
	cleanups, err := st.ListImageCleanups()
	if err != nil {
		t.Fatal(err)
	}
	if len(cleanups) != 0 {
		t.Fatalf("unexpected pending cleanup: %+v", cleanups)
	}
	if _, err := st.GetImage(old.Name); err == nil {
		t.Fatal("removed alias still exists")
	}
	if _, err := st.GetImage(live.Name); err != nil {
		t.Fatalf("live alias disappeared: %v", err)
	}
}

func TestDeleteImageIfMatchWithCleanupRejectsSameIDDifferentRootFS(t *testing.T) {
	base := t.TempDir()
	st, err := Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	first := &Image{ID: "conflicting-cleanup-id", Name: "conflict:first", Tag: "first", RootFS: filepath.Join(base, "images", "conflicting-cleanup-id", "rootfs")}
	second := &Image{ID: "conflicting-cleanup-id", Name: "conflict:second", Tag: "second", RootFS: filepath.Join(base, "images", "other-generation", "rootfs")}
	if err := st.SaveImage(first); err != nil {
		t.Fatal(err)
	}
	writeCurrentImageMetadata(t, st, second)
	expected, err := st.GetImage(first.Name)
	if err != nil {
		t.Fatal(err)
	}
	cleanup := ImageCleanup{ID: expected.ID, RootFS: expected.RootFS}

	if _, armed, err := st.DeleteImageIfMatchWithCleanup(first.Name, expected, cleanup); err == nil || !strings.Contains(err.Error(), "inconsistent image aliases") {
		t.Fatalf("DeleteImageIfMatchWithCleanup err=%v armed=%v, want inconsistent-alias rejection", err, armed)
	}
	if _, err := st.GetImage(first.Name); err != nil {
		t.Fatalf("first metadata disappeared after rejected cleanup: %v", err)
	}
	if _, err := st.GetImage(second.Name); err != nil {
		t.Fatalf("second metadata disappeared after rejected cleanup: %v", err)
	}
	cleanups, err := st.ListImageCleanups()
	if err != nil {
		t.Fatal(err)
	}
	if len(cleanups) != 0 {
		t.Fatalf("cleanup armed despite inconsistent aliases: %+v", cleanups)
	}
}

func TestImageCleanupSidecarSurvivesStoreReopen(t *testing.T) {
	base := t.TempDir()
	st, err := Open(base)
	if err != nil {
		t.Fatal(err)
	}
	rootFS := filepath.Join(base, "images", "reopen-cleanup-id", "rootfs")
	img := &Image{ID: "reopen-cleanup-id", Name: "reopen:old", Tag: "<none>", RootFS: rootFS}
	if err := st.SaveImage(img); err != nil {
		t.Fatal(err)
	}
	expected, err := st.GetImage(img.Name)
	if err != nil {
		t.Fatal(err)
	}
	cleanup := ImageCleanup{ID: expected.ID, RootFS: expected.RootFS}
	if _, armed, err := st.DeleteImageIfMatchWithCleanup(img.Name, expected, cleanup); err != nil || !armed {
		t.Fatalf("arm cleanup: armed=%v err=%v", armed, err)
	}
	if err := st.Close(); err != nil {
		t.Fatal(err)
	}

	reopened, err := Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	cleanups, err := reopened.ListImageCleanups()
	if err != nil {
		t.Fatal(err)
	}
	if len(cleanups) != 1 || cleanups[0] != cleanup {
		t.Fatalf("reopened cleanups=%+v, want [%+v]", cleanups, cleanup)
	}
}
