//go:build linux

package container

import (
	"crypto/sha256"
	"encoding/hex"
	"net"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func saveCommitTestContainer(t *testing.T, st *state.Store, id, rootFS string) {
	t.Helper()
	if err := st.Save(&state.Container{
		ID:        id,
		Status:    state.StatusStopped,
		RootFS:    rootFS,
		Command:   []string{"true"},
		CreatedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}
}

func makeCommitSource(t *testing.T) string {
	t.Helper()
	rootFS := t.TempDir()
	if err := os.WriteFile(filepath.Join(rootFS, "payload.txt"), []byte("commit payload\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	return rootFS
}

func TestCommitContainerRejectsReplacedStateRoot(t *testing.T) {
	parent := t.TempDir()
	stateRoot := filepath.Join(parent, "state")
	st, err := state.Open(stateRoot)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	source := makeCommitSource(t)
	saveCommitTestContainer(t, st, "commit-replaced-root", source)

	originalRoot := filepath.Join(parent, "state-original")
	if err := os.Rename(stateRoot, originalRoot); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(stateRoot, "containers"), 0o700); err != nil {
		t.Fatal(err)
	}
	replacementImages := filepath.Join(stateRoot, "images")
	if err := os.MkdirAll(replacementImages, 0o700); err != nil {
		t.Fatal(err)
	}

	if _, err := CommitContainer(st, "commit-replaced-root", "replacement:latest"); err == nil || !strings.Contains(err.Error(), "changed generation") {
		t.Fatalf("replaced state-root commit error = %v", err)
	}
	entries, err := os.ReadDir(replacementImages)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 0 {
		t.Fatalf("commit wrote into replacement image directory: %v", entries)
	}
	originalEntries, err := os.ReadDir(filepath.Join(originalRoot, "images"))
	if err != nil {
		t.Fatal(err)
	}
	if len(originalEntries) != 0 {
		t.Fatalf("failed acquisition still created pinned image artifacts: %v", originalEntries)
	}
}

func TestCommitContainerRejectsReplacedImageDirectory(t *testing.T) {
	stateRoot := filepath.Join(t.TempDir(), "state")
	st, err := state.Open(stateRoot)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	source := makeCommitSource(t)
	saveCommitTestContainer(t, st, "commit-replaced-images", source)

	imagesPath := filepath.Join(stateRoot, "images")
	originalImages := filepath.Join(stateRoot, "images-original")
	if err := os.Rename(imagesPath, originalImages); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(imagesPath, 0o700); err != nil {
		t.Fatal(err)
	}

	if _, err := CommitContainer(st, "commit-replaced-images", "replacement-images:latest"); err == nil || !strings.Contains(err.Error(), "image directory changed generation") {
		t.Fatalf("replaced image-dir commit error = %v", err)
	}
	entries, err := os.ReadDir(imagesPath)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 0 {
		t.Fatalf("commit wrote into replacement image directory: %v", entries)
	}
	originalEntries, err := os.ReadDir(originalImages)
	if err != nil {
		t.Fatal(err)
	}
	if len(originalEntries) != 0 {
		t.Fatalf("failed acquisition still created pinned image artifacts: %v", originalEntries)
	}
}

func TestCommitContainerExportFailureCleansStaging(t *testing.T) {
	stateRoot := filepath.Join(t.TempDir(), "state")
	st, err := state.Open(stateRoot)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	rootFS := t.TempDir()
	if err := os.WriteFile(filepath.Join(rootFS, "payload.txt"), []byte("payload\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	listener, err := net.Listen("unix", filepath.Join(rootFS, "unsupported.sock"))
	if err != nil {
		t.Fatal(err)
	}
	defer listener.Close()
	saveCommitTestContainer(t, st, "commit-export-fail", rootFS)

	if _, err := CommitContainer(st, "commit-export-fail", "export-fails:latest"); err == nil || !strings.Contains(err.Error(), "export container rootfs") {
		t.Fatalf("export failure error = %v", err)
	}
	entries, err := os.ReadDir(filepath.Join(stateRoot, "images"))
	if err != nil {
		t.Fatal(err)
	}
	for _, entry := range entries {
		if entry.IsDir() && strings.HasPrefix(entry.Name(), ".commit-") {
			t.Fatalf("failed commit left staging directory %q", entry.Name())
		}
	}
	if _, err := st.GetImage("export-fails:latest"); err == nil {
		t.Fatal("failed export still persisted image metadata")
	}
}

func TestCommitContainerMetadataFailureRollsBackPublishedPayload(t *testing.T) {
	stateRoot := filepath.Join(t.TempDir(), "state")
	st, err := state.Open(stateRoot)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	source := makeCommitSource(t)
	saveCommitTestContainer(t, st, "commit-metadata-fail", source)

	const tag = "metadata-fails:latest"
	keyDigest := sha256.Sum256([]byte(tag))
	blockerName := "img-" + hex.EncodeToString(keyDigest[:]) + ".json"
	blocker := filepath.Join(stateRoot, "images", blockerName)
	if err := os.Mkdir(blocker, 0o700); err != nil {
		t.Fatal(err)
	}

	if _, err := CommitContainer(st, "commit-metadata-fail", tag); err == nil || !strings.Contains(err.Error(), "save image record") {
		t.Fatalf("metadata failure error = %v", err)
	}
	entries, err := os.ReadDir(filepath.Join(stateRoot, "images"))
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 1 || entries[0].Name() != blockerName || !entries[0].IsDir() {
		t.Fatalf("metadata failure left image payload/staging artifacts: %v", entries)
	}
	if _, err := st.GetImage(tag); err == nil {
		t.Fatal("failed metadata commit unexpectedly produced image record")
	}
}
