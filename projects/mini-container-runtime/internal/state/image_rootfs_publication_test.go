package state

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestSaveImageRejectsInvalidNonEmptyRootFSBeforePublication(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}

	tests := []struct {
		name    string
		rootfs  string
		wantErr string
	}{
		{name: "whitespace", rootfs: "   ", wantErr: "whitespace-only"},
		{name: "relative", rootfs: "rootfs/image", wantErr: "must be absolute"},
		{name: "dot segment", rootfs: "/srv/images/./rootfs", wantErr: "must be clean"},
		{name: "parent segment", rootfs: "/srv/images/../rootfs", wantErr: "must be clean"},
		{name: "trailing slash", rootfs: "/srv/images/rootfs/", wantErr: "must be clean"},
		{name: "nul", rootfs: "/srv/images/rootfs\x00suffix", wantErr: "NUL byte"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			img := &Image{Name: "repo:" + strings.ReplaceAll(tt.name, " ", "-"), ID: "id-" + strings.ReplaceAll(tt.name, " ", "-"), RootFS: tt.rootfs}
			err := store.SaveImage(img)
			if err == nil || !strings.Contains(err.Error(), tt.wantErr) {
				t.Fatalf("SaveImage error=%v, want substring %q", err, tt.wantErr)
			}
			path := filepath.Join(store.imgDir, imageMetadataFilename(img.Name))
			if _, statErr := os.Lstat(path); !os.IsNotExist(statErr) {
				t.Fatalf("invalid image metadata was published: stat err=%v", statErr)
			}
		})
	}
}

func TestSaveImageAllowsEmptyRootFSSentinel(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}

	img := &Image{Name: "repo:metadata-only", ID: "id-metadata-only"}
	if err := store.SaveImage(img); err != nil {
		t.Fatalf("SaveImage metadata-only image: %v", err)
	}
}

func TestSaveImageAllowsCleanAbsoluteRootFSWithoutEagerFilesystemLookup(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}

	rootfs := filepath.Join(t.TempDir(), "not-created-yet")
	img := &Image{Name: "repo:clean", ID: "id-clean", RootFS: rootfs}
	if err := store.SaveImage(img); err != nil {
		t.Fatalf("SaveImage clean absolute rootfs: %v", err)
	}

	got, err := store.GetImage(img.Name)
	if err != nil {
		t.Fatalf("GetImage: %v", err)
	}
	if got.RootFS != rootfs {
		t.Fatalf("RootFS=%q, want %q", got.RootFS, rootfs)
	}
}
