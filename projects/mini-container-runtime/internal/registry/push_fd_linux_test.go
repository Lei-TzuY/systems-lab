//go:build linux

package registry

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"golang.org/x/sys/unix"
)

func TestBuildOCILayerDoesNotAccumulateOpenFiles(t *testing.T) {
	base := t.TempDir()
	rootfs := filepath.Join(base, "rootfs")
	if err := os.Mkdir(rootfs, 0o755); err != nil {
		t.Fatal(err)
	}
	// More regular files than the temporary soft RLIMIT below. An implementation
	// that defers every file.Close until the walk finishes will exhaust FDs.
	for i := 0; i < 96; i++ {
		name := filepath.Join(rootfs, fmt.Sprintf("file-%03d", i))
		if err := os.WriteFile(name, []byte("x"), 0o644); err != nil {
			t.Fatal(err)
		}
	}

	var old unix.Rlimit
	if err := unix.Getrlimit(unix.RLIMIT_NOFILE, &old); err != nil {
		t.Skipf("get RLIMIT_NOFILE: %v", err)
	}
	if old.Cur < 64 {
		t.Skipf("existing RLIMIT_NOFILE too small for controlled test: %d", old.Cur)
	}
	limited := old
	limited.Cur = 64
	if err := unix.Setrlimit(unix.RLIMIT_NOFILE, &limited); err != nil {
		t.Skipf("set RLIMIT_NOFILE: %v", err)
	}
	defer func() {
		if err := unix.Setrlimit(unix.RLIMIT_NOFILE, &old); err != nil {
			t.Errorf("restore RLIMIT_NOFILE: %v", err)
		}
	}()

	archive := filepath.Join(base, "layer.tar.gz")
	if _, _, err := BuildOCILayer(rootfs, archive); err != nil {
		t.Fatalf("BuildOCILayer under bounded RLIMIT_NOFILE: %v", err)
	}
}
