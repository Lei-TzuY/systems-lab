package state

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestImageIdentityBudgetBoundaryCountsBytes(t *testing.T) {
	exact := strings.Repeat("x", maxImageIdentityBytes)
	if err := validateImageSelector(exact); err != nil {
		t.Fatalf("exact image identity budget rejected: %v", err)
	}
	if err := validateImageSelector(exact + "x"); err == nil {
		t.Fatal("image identity over byte budget was accepted")
	}

	multibyte := strings.Repeat("界", maxImageIdentityBytes/3)
	multibyte += strings.Repeat("x", maxImageIdentityBytes-len(multibyte))
	if len(multibyte) != maxImageIdentityBytes {
		t.Fatalf("multibyte fixture length=%d, want %d", len(multibyte), maxImageIdentityBytes)
	}
	if err := validateImageSelector(multibyte); err != nil {
		t.Fatalf("exact multibyte image identity budget rejected: %v", err)
	}
	if err := validateImageSelector(multibyte + "界"); err == nil {
		t.Fatal("multibyte image identity over byte budget was accepted")
	}
}

func TestSaveImageRejectsOverlongIdentityFieldsBeforeMutation(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()

	overlong := strings.Repeat("x", maxImageIdentityBytes+1)
	cases := []Image{
		{Name: overlong, ID: "valid-id", RootFS: "/rootfs"},
		{Name: "valid-name", ID: overlong, RootFS: "/rootfs"},
	}
	for _, img := range cases {
		if err := store.SaveImage(&img); err == nil {
			t.Fatalf("SaveImage accepted overlong identity: name=%d id=%d", len(img.Name), len(img.ID))
		}
		entries, err := os.ReadDir(store.imgDir)
		if err != nil {
			t.Fatal(err)
		}
		if len(entries) != 0 {
			t.Fatalf("rejected SaveImage mutated image state: %v", entries)
		}
	}
}

func TestImageSelectorsRejectOverlongInputBeforeStateRead(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()

	if err := os.WriteFile(filepath.Join(store.imgDir, "corrupt.json"), []byte("not-json"), 0o600); err != nil {
		t.Fatal(err)
	}
	overlong := strings.Repeat("x", maxImageIdentityBytes+1)

	if _, err := store.GetImage(overlong); err == nil || !strings.Contains(err.Error(), "exceeds") {
		t.Fatalf("GetImage overlong selector err=%v, want byte-budget rejection", err)
	}
	if _, err := store.GetImageUnlocked(overlong); err == nil || !strings.Contains(err.Error(), "exceeds") {
		t.Fatalf("GetImageUnlocked overlong selector err=%v, want byte-budget rejection", err)
	}
	if _, err := store.DeleteImage(overlong); err == nil || !strings.Contains(err.Error(), "exceeds") {
		t.Fatalf("DeleteImage overlong selector err=%v, want byte-budget rejection", err)
	}
}

func TestListImagesRejectsOverlongEmbeddedIdentity(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()

	name := "valid-name"
	overlongID := strings.Repeat("i", maxImageIdentityBytes+1)
	data := []byte("{\"id\":\"" + overlongID + "\",\"name\":\"" + name + "\",\"rootfs\":\"/rootfs\",\"loaded_at\":\"0001-01-01T00:00:00Z\"}")
	path := filepath.Join(store.imgDir, imageMetadataFilename(name))
	if err := os.WriteFile(path, data, 0o600); err != nil {
		t.Fatal(err)
	}

	if _, err := store.ListImages(); err == nil || !strings.Contains(err.Error(), "invalid image ID") {
		t.Fatalf("ListImages err=%v, want embedded ID byte-budget rejection", err)
	}
}