package system

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func openDFSizeErrorStore(t *testing.T) (*state.Store, string) {
	t.Helper()
	base := t.TempDir()
	home := filepath.Join(base, "home")
	if err := os.MkdirAll(home, 0o700); err != nil {
		t.Fatal(err)
	}
	t.Setenv("HOME", home)
	st, err := state.Open(filepath.Join(base, "store"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = st.Close() })
	return st, base
}

func TestCalculateDFSurfacesContainerRootFSSizeFailure(t *testing.T) {
	st, base := openDFSizeErrorStore(t)
	missing := filepath.Join(base, "missing-container-rootfs")
	if err := st.Save(&state.Container{
		ID:        "broken-df-container",
		Status:    state.StatusStopped,
		RootFS:    missing,
		CreatedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}

	res, err := CalculateDF(st)
	if err == nil {
		t.Fatal("CalculateDF unexpectedly hid missing container rootfs")
	}
	if res == nil || res.ContainersCount != 1 {
		t.Fatalf("partial df result=%+v, want one container", res)
	}
	if !strings.Contains(err.Error(), "broken-df-container") || !strings.Contains(err.Error(), missing) {
		t.Fatalf("container size error=%v", err)
	}
}

func TestCalculateDFSurfacesZeroSizeImageRootFSFailure(t *testing.T) {
	st, base := openDFSizeErrorStore(t)
	missing := filepath.Join(base, "missing-image-rootfs")
	if err := st.SaveImage(&state.Image{
		ID:       "broken-df-image",
		Name:     "broken:latest",
		Tag:      "latest",
		RootFS:   missing,
		Size:     0,
		LoadedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}

	res, err := CalculateDF(st)
	if err == nil {
		t.Fatal("CalculateDF unexpectedly hid missing zero-size image rootfs")
	}
	if res == nil || res.ImagesCount != 1 {
		t.Fatalf("partial df result=%+v, want one image", res)
	}
	if !strings.Contains(err.Error(), "broken:latest") || !strings.Contains(err.Error(), missing) {
		t.Fatalf("image size error=%v", err)
	}
}
