//go:build linux

package state

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestImageStorageLeaseRejectsConfiguredRootReplacement(t *testing.T) {
	parent := t.TempDir()
	root := filepath.Join(parent, "state")
	st, err := Open(root)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	lease, err := st.AcquireImageStorage()
	if err != nil {
		t.Fatalf("AcquireImageStorage: %v", err)
	}
	defer lease.Close()
	if err := lease.ValidateConfiguredGeneration(); err != nil {
		t.Fatalf("initial generation validation: %v", err)
	}

	originalRoot := filepath.Join(parent, "state-original")
	if err := os.Rename(root, originalRoot); err != nil {
		t.Fatal(err)
	}
	outside := t.TempDir()
	if err := os.Symlink(outside, root); err != nil {
		t.Fatal(err)
	}

	if err := lease.ValidateConfiguredGeneration(); err == nil || !strings.Contains(err.Error(), "real directory") {
		t.Fatalf("root replacement validation error=%v", err)
	}
	if _, err := os.Stat(filepath.Join(outside, "images")); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("lease validation created replacement image storage: %v", err)
	}
}

func TestImageStorageLeaseRejectsConfiguredImageDirectoryReplacement(t *testing.T) {
	root := filepath.Join(t.TempDir(), "state")
	st, err := Open(root)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	lease, err := st.AcquireImageStorage()
	if err != nil {
		t.Fatal(err)
	}
	defer lease.Close()

	images := filepath.Join(root, "images")
	originalImages := filepath.Join(root, "images-original")
	if err := os.Rename(images, originalImages); err != nil {
		t.Fatal(err)
	}
	outside := t.TempDir()
	if err := os.Symlink(outside, images); err != nil {
		t.Fatal(err)
	}

	if err := lease.ValidateConfiguredGeneration(); err == nil || !strings.Contains(err.Error(), "real directory") {
		t.Fatalf("image replacement validation error=%v", err)
	}
}

func TestImageStorageLeaseOwnsIndependentHandlesAcrossStoreClose(t *testing.T) {
	root := filepath.Join(t.TempDir(), "state")
	st, err := Open(root)
	if err != nil {
		t.Fatal(err)
	}
	lease, err := st.AcquireImageStorage()
	if err != nil {
		t.Fatal(err)
	}
	defer lease.Close()

	if err := st.Close(); err != nil {
		t.Fatalf("Store.Close: %v", err)
	}

	const payload = "lease-still-pinned"
	if err := os.WriteFile(filepath.Join(lease.Path(), "lease-sentinel"), []byte(payload), 0o600); err != nil {
		t.Fatalf("write through independent lease after Store.Close: %v", err)
	}
	got, err := os.ReadFile(filepath.Join(root, "images", "lease-sentinel"))
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != payload {
		t.Fatalf("sentinel=%q, want %q", got, payload)
	}
}

func TestAcquireImageStorageRejectsClosedStore(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Close(); err != nil {
		t.Fatal(err)
	}
	if _, err := st.AcquireImageStorage(); !errors.Is(err, ErrStoreClosed) {
		t.Fatalf("AcquireImageStorage after Close error=%v, want ErrStoreClosed", err)
	}
}
