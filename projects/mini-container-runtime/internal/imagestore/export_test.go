package imagestore

import (
	"os"
	"path/filepath"
	"testing"

	"minicontainer/internal/state"
)

func TestExportContainerRootFS(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	rootFS := filepath.Join(tmpDir, "rootfs")
	_ = os.MkdirAll(rootFS, 0755)
	_ = os.WriteFile(filepath.Join(rootFS, "hello.txt"), []byte("export data"), 0644)

	c := &state.Container{
		ID:     "ctr-exp-1",
		Status: state.StatusStopped,
		RootFS: rootFS,
	}
	_ = st.Save(c)

	outTar := filepath.Join(tmpDir, "export.tar.gz")
	if err := ExportContainerRootFS(st, c.ID, outTar); err != nil {
		t.Fatalf("ExportContainerRootFS error: %v", err)
	}

	if info, err := os.Stat(outTar); err != nil || info.Size() == 0 {
		t.Fatalf("Exported tarball missing or empty: %v", err)
	}
}
