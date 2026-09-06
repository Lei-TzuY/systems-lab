package state

import (
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestPublishImagePreservesDisplacedLastPayloadAsDangling(t *testing.T) {
	base := t.TempDir()
	st, err := Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	oldRoot := filepath.Join(base, "images", "old-id", "rootfs")
	old := &Image{ID: "old-id", Name: "app:latest", Repository: "app", Tag: "latest", RootFS: oldRoot, Size: 10, LoadedAt: time.Now()}
	if err := st.PublishImage(old); err != nil {
		t.Fatal(err)
	}

	newRoot := filepath.Join(base, "images", "new-id", "rootfs")
	newImg := &Image{ID: "new-id", Name: "app:latest", Repository: "app", Tag: "latest", RootFS: newRoot, Size: 20, LoadedAt: time.Now()}
	if err := st.PublishImage(newImg); err != nil {
		t.Fatalf("PublishImage replacement: %v", err)
	}

	current, err := st.GetImage("app:latest")
	if err != nil {
		t.Fatal(err)
	}
	if current.ID != newImg.ID || filepath.Clean(current.RootFS) != filepath.Clean(newRoot) {
		t.Fatalf("current tag=%+v, want new payload", current)
	}
	dangling, err := st.GetImage(old.ID)
	if err != nil {
		t.Fatalf("old payload lost after replacement: %v", err)
	}
	if dangling.ID != old.ID || dangling.Name != "" || dangling.Tag != "<none>" || filepath.Clean(dangling.RootFS) != filepath.Clean(oldRoot) {
		t.Fatalf("dangling record=%+v, want old payload ownership", dangling)
	}
}

func TestPublishImageDoesNotCreateDanglingWhenOldPayloadStillAliased(t *testing.T) {
	base := t.TempDir()
	st, err := Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	oldRoot := filepath.Join(base, "images", "shared-old-id", "rootfs")
	latest := &Image{ID: "shared-old-id", Name: "app:latest", Tag: "latest", RootFS: oldRoot, LoadedAt: time.Now()}
	stable := &Image{ID: "shared-old-id", Name: "app:stable", Tag: "stable", RootFS: oldRoot, LoadedAt: time.Now()}
	if err := st.PublishImage(latest); err != nil {
		t.Fatal(err)
	}
	if err := st.PublishImage(stable); err != nil {
		t.Fatal(err)
	}

	newImg := &Image{ID: "replacement-id", Name: "app:latest", Tag: "latest", RootFS: filepath.Join(base, "images", "replacement-id", "rootfs"), LoadedAt: time.Now()}
	if err := st.PublishImage(newImg); err != nil {
		t.Fatal(err)
	}
	if _, err := st.GetImage("app:stable"); err != nil {
		t.Fatalf("existing alias disappeared: %v", err)
	}
	images, err := st.ListImages()
	if err != nil {
		t.Fatal(err)
	}
	if len(images) != 2 {
		t.Fatalf("image records=%d, want stable alias + replacement only: %+v", len(images), images)
	}
	for _, img := range images {
		if img != nil && img.ID == latest.ID && img.Name == "" && img.Tag == "<none>" {
			t.Fatalf("replacement created redundant dangling metadata despite live alias: %+v", img)
		}
	}
}

func TestPublishImageSamePayloadUpdatesMetadataWithoutDangling(t *testing.T) {
	base := t.TempDir()
	st, err := Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	rootFS := filepath.Join(base, "images", "same-id", "rootfs")
	first := &Image{ID: "same-id", Name: "same:latest", Tag: "latest", RootFS: rootFS, Size: 10, LoadedAt: time.Now()}
	if err := st.PublishImage(first); err != nil {
		t.Fatal(err)
	}
	updated := *first
	updated.Size = 99
	updated.WorkDir = "/work"
	if err := st.PublishImage(&updated); err != nil {
		t.Fatalf("same-payload update: %v", err)
	}

	got, err := st.GetImage(first.Name)
	if err != nil {
		t.Fatal(err)
	}
	if got.Size != 99 || got.WorkDir != "/work" {
		t.Fatalf("updated metadata=%+v", got)
	}
	images, err := st.ListImages()
	if err != nil {
		t.Fatal(err)
	}
	if len(images) != 1 {
		t.Fatalf("same-payload update created extra records: %+v", images)
	}
}

func TestPublishImageRejectsSameIDDifferentRootFS(t *testing.T) {
	base := t.TempDir()
	st, err := Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	first := &Image{ID: "same-id-conflict", Name: "conflict:latest", Tag: "latest", RootFS: filepath.Join(base, "images", "a", "rootfs"), LoadedAt: time.Now()}
	if err := st.PublishImage(first); err != nil {
		t.Fatal(err)
	}
	conflict := *first
	conflict.RootFS = filepath.Join(base, "images", "b", "rootfs")
	if err := st.PublishImage(&conflict); err == nil || !strings.Contains(err.Error(), "existing alias references") {
		t.Fatalf("same-ID different-rootfs publication error=%v", err)
	}
	got, err := st.GetImage(first.Name)
	if err != nil {
		t.Fatal(err)
	}
	if filepath.Clean(got.RootFS) != filepath.Clean(first.RootFS) {
		t.Fatalf("conflicting publication changed durable metadata: %+v", got)
	}
}

func TestPublishImageRejectsIncomingIDConflictingWithOtherAlias(t *testing.T) {
	base := t.TempDir()
	st, err := Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	existing := &Image{
		ID:       "shared-id-conflict",
		Name:     "existing:stable",
		Tag:      "stable",
		RootFS:   filepath.Join(base, "images", "stable", "rootfs"),
		LoadedAt: time.Now(),
	}
	if err := st.SaveImage(existing); err != nil {
		t.Fatal(err)
	}
	incoming := &Image{
		ID:       existing.ID,
		Name:     "incoming:latest",
		Tag:      "latest",
		RootFS:   filepath.Join(base, "images", "different", "rootfs"),
		LoadedAt: time.Now(),
	}
	if err := st.PublishImage(incoming); err == nil || !strings.Contains(err.Error(), "existing alias references") {
		t.Fatalf("incoming conflicting-ID publication error=%v", err)
	}
	if _, err := st.GetImage(incoming.Name); err == nil {
		t.Fatal("conflicting incoming image was published")
	}
	got, err := st.GetImage(existing.Name)
	if err != nil {
		t.Fatal(err)
	}
	if filepath.Clean(got.RootFS) != filepath.Clean(existing.RootFS) {
		t.Fatalf("existing alias changed after rejected publication: %+v", got)
	}
}
