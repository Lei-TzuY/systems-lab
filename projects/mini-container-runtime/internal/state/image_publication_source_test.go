package state

import (
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestPublishImageIfSourceMatchPublishesUnchangedSource(t *testing.T) {
	base := t.TempDir()
	st, err := Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	source := &Image{
		ID:       "source-cas-id",
		Name:     "app:v1",
		Tag:      "v1",
		RootFS:   filepath.Join(base, "images", "source-cas-id", "rootfs"),
		LoadedAt: time.Now(),
	}
	if err := st.PublishImage(source); err != nil {
		t.Fatal(err)
	}
	snapshot, err := st.GetImage(source.Name)
	if err != nil {
		t.Fatal(err)
	}
	target := *snapshot
	target.Name = "app:latest"
	target.Tag = "latest"

	if err := st.PublishImageIfSourceMatch(source.Name, snapshot, &target); err != nil {
		t.Fatalf("PublishImageIfSourceMatch: %v", err)
	}
	got, err := st.GetImage(target.Name)
	if err != nil {
		t.Fatal(err)
	}
	if !sameImagePayload(got, snapshot) {
		t.Fatalf("published target=%+v, want source payload %+v", got, snapshot)
	}
}

func TestPublishImageIfSourceMatchRejectsChangedSource(t *testing.T) {
	base := t.TempDir()
	st, err := Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	source := &Image{
		ID:       "changed-source-id",
		Name:     "changed:v1",
		Tag:      "v1",
		RootFS:   filepath.Join(base, "images", "changed-source-id", "rootfs"),
		LoadedAt: time.Now(),
	}
	if err := st.PublishImage(source); err != nil {
		t.Fatal(err)
	}
	snapshot, err := st.GetImage(source.Name)
	if err != nil {
		t.Fatal(err)
	}
	changed := *snapshot
	changed.WorkDir = "/new-workdir"
	if err := st.SaveImage(&changed); err != nil {
		t.Fatal(err)
	}
	target := *snapshot
	target.Name = "changed:latest"
	target.Tag = "latest"

	if err := st.PublishImageIfSourceMatch(source.Name, snapshot, &target); err == nil || !strings.Contains(err.Error(), "changed before tag publication") {
		t.Fatalf("stale source publication error=%v", err)
	}
	if _, err := st.GetImage(target.Name); err == nil {
		t.Fatal("target was published from stale source snapshot")
	}
	current, err := st.GetImage(source.Name)
	if err != nil {
		t.Fatal(err)
	}
	if current.WorkDir != changed.WorkDir {
		t.Fatalf("source changed unexpectedly: %+v", current)
	}
}

func TestPublishImageIfSourceMatchRejectsDeletedSource(t *testing.T) {
	base := t.TempDir()
	st, err := Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	source := &Image{
		ID:       "deleted-source-id",
		Name:     "deleted:v1",
		Tag:      "v1",
		RootFS:   filepath.Join(base, "images", "deleted-source-id", "rootfs"),
		LoadedAt: time.Now(),
	}
	if err := st.PublishImage(source); err != nil {
		t.Fatal(err)
	}
	snapshot, err := st.GetImage(source.Name)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := st.DeleteImage(source.Name); err != nil {
		t.Fatal(err)
	}
	target := *snapshot
	target.Name = "deleted:latest"
	target.Tag = "latest"

	if err := st.PublishImageIfSourceMatch(source.Name, snapshot, &target); err == nil || !strings.Contains(err.Error(), "changed before tag publication") {
		t.Fatalf("deleted source publication error=%v", err)
	}
	if _, err := st.GetImage(target.Name); err == nil {
		t.Fatal("target was published after source metadata deletion")
	}
}

func TestPublishImageIfSourceMatchRejectsDifferentPayload(t *testing.T) {
	base := t.TempDir()
	st, err := Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	source := &Image{
		ID:       "payload-source-id",
		Name:     "payload:v1",
		Tag:      "v1",
		RootFS:   filepath.Join(base, "images", "payload-source-id", "rootfs"),
		LoadedAt: time.Now(),
	}
	if err := st.PublishImage(source); err != nil {
		t.Fatal(err)
	}
	snapshot, err := st.GetImage(source.Name)
	if err != nil {
		t.Fatal(err)
	}
	target := *snapshot
	target.Name = "payload:latest"
	target.Tag = "latest"
	target.ID = "different-payload-id"
	target.RootFS = filepath.Join(base, "images", target.ID, "rootfs")

	if err := st.PublishImageIfSourceMatch(source.Name, snapshot, &target); err == nil || !strings.Contains(err.Error(), "does not alias the expected source payload") {
		t.Fatalf("different-payload publication error=%v", err)
	}
	if _, err := st.GetImage(target.Name); err == nil {
		t.Fatal("different payload was published as source alias")
	}
}

func TestPublishImageIfSourceMatchPreservesDisplacedTargetPayload(t *testing.T) {
	base := t.TempDir()
	st, err := Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	old := &Image{
		ID:       "old-target-id",
		Name:     "app:latest",
		Tag:      "latest",
		RootFS:   filepath.Join(base, "images", "old-target-id", "rootfs"),
		LoadedAt: time.Now(),
	}
	source := &Image{
		ID:       "source-target-id",
		Name:     "app:v2",
		Tag:      "v2",
		RootFS:   filepath.Join(base, "images", "source-target-id", "rootfs"),
		LoadedAt: time.Now(),
	}
	if err := st.PublishImage(old); err != nil {
		t.Fatal(err)
	}
	if err := st.PublishImage(source); err != nil {
		t.Fatal(err)
	}
	snapshot, err := st.GetImage(source.Name)
	if err != nil {
		t.Fatal(err)
	}
	target := *snapshot
	target.Name = old.Name
	target.Tag = old.Tag

	if err := st.PublishImageIfSourceMatch(source.Name, snapshot, &target); err != nil {
		t.Fatalf("PublishImageIfSourceMatch replacement: %v", err)
	}
	dangling, err := st.GetImage(old.ID)
	if err != nil {
		t.Fatalf("displaced target payload lost: %v", err)
	}
	if dangling.Name != "" || dangling.Tag != "<none>" || !sameImagePayload(dangling, old) {
		t.Fatalf("displaced target dangling record=%+v, want %+v payload", dangling, old)
	}
}
