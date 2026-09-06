package imagestore

import (
	"errors"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

type sizeTestDirEntry struct {
	name    string
	size    int64
	infoErr error
}

func (d sizeTestDirEntry) Name() string               { return d.name }
func (d sizeTestDirEntry) IsDir() bool                { return false }
func (d sizeTestDirEntry) Type() fs.FileMode          { return 0 }
func (d sizeTestDirEntry) Info() (fs.FileInfo, error) {
	if d.infoErr != nil {
		return nil, d.infoErr
	}
	return sizeTestFileInfo{name: d.name, size: d.size}, nil
}

type sizeTestFileInfo struct {
	name string
	size int64
}

func (i sizeTestFileInfo) Name() string       { return i.name }
func (i sizeTestFileInfo) Size() int64        { return i.size }
func (i sizeTestFileInfo) Mode() fs.FileMode  { return 0o600 }
func (i sizeTestFileInfo) ModTime() time.Time { return time.Time{} }
func (i sizeTestFileInfo) IsDir() bool        { return false }
func (i sizeTestFileInfo) Sys() any           { return nil }

func TestCalculateDirSizeMissingRootFailsClosed(t *testing.T) {
	missing := filepath.Join(t.TempDir(), "missing")
	size, err := CalculateDirSize(missing)
	if err == nil {
		t.Fatal("CalculateDirSize unexpectedly succeeded for missing root")
	}
	if size != 0 {
		t.Fatalf("size=%d after failed traversal, want 0", size)
	}
	if !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("missing-root error=%v, want os.ErrNotExist", err)
	}
	if !strings.Contains(err.Error(), missing) {
		t.Fatalf("missing-root error %q does not identify %q", err, missing)
	}
}

func TestCalculateDirSizeDiscardsPartialSizeOnInfoError(t *testing.T) {
	infoErr := errors.New("injected info failure")
	root := filepath.Join("virtual", "root")
	walk := func(_ string, fn fs.WalkDirFunc) error {
		if err := fn(filepath.Join(root, "first.bin"), sizeTestDirEntry{name: "first.bin", size: 17}, nil); err != nil {
			return err
		}
		return fn(filepath.Join(root, "broken.bin"), sizeTestDirEntry{name: "broken.bin", infoErr: infoErr}, nil)
	}

	size, err := calculateDirSizeWithWalk(root, walk)
	if err == nil {
		t.Fatal("calculateDirSizeWithWalk unexpectedly succeeded after Info failure")
	}
	if size != 0 {
		t.Fatalf("size=%d after partial traversal failure, want 0", size)
	}
	if !errors.Is(err, infoErr) {
		t.Fatalf("Info error=%v, want injected error", err)
	}
	if !strings.Contains(err.Error(), "broken.bin") {
		t.Fatalf("Info error %q does not identify failed entry", err)
	}
}

func TestCalculateDirSizePropagatesWalkErrorWithoutPartialSize(t *testing.T) {
	walkErr := errors.New("injected walk failure")
	root := filepath.Join("virtual", "root")
	walk := func(_ string, fn fs.WalkDirFunc) error {
		if err := fn(filepath.Join(root, "first.bin"), sizeTestDirEntry{name: "first.bin", size: 11}, nil); err != nil {
			return err
		}
		return fn(filepath.Join(root, "blocked"), nil, walkErr)
	}

	size, err := calculateDirSizeWithWalk(root, walk)
	if err == nil {
		t.Fatal("calculateDirSizeWithWalk unexpectedly succeeded after walk failure")
	}
	if size != 0 {
		t.Fatalf("size=%d after walk failure, want 0", size)
	}
	if !errors.Is(err, walkErr) {
		t.Fatalf("walk error=%v, want injected error", err)
	}
	if !strings.Contains(err.Error(), "blocked") {
		t.Fatalf("walk error %q does not identify failed path", err)
	}
}
