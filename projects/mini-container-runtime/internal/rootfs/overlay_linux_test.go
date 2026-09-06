//go:build linux

package rootfs

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestValidateOverlayOptionPathRejectsMountDelimiters(t *testing.T) {
	tests := []struct {
		name     string
		path     string
		lowerdir bool
	}{
		{name: "comma", path: "/tmp/lower,upperdir=/tmp/evil", lowerdir: true},
		{name: "backslash", path: `/tmp/lower\,upperdir=/tmp/evil`, lowerdir: true},
		{name: "lowerdir colon", path: "/tmp/lower:/tmp/evil", lowerdir: true},
		{name: "upper comma", path: "/tmp/upper,workdir=/tmp/evil", lowerdir: false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if err := validateOverlayOptionPath("test", tt.path, tt.lowerdir); err == nil {
				t.Fatalf("validateOverlayOptionPath(%q) unexpectedly succeeded", tt.path)
			}
		})
	}
}

func TestValidateOverlayOptionPathAllowsColonOutsideLowerdir(t *testing.T) {
	if err := validateOverlayOptionPath("upper", "/tmp/upper:allowed", false); err != nil {
		t.Fatalf("upper path with colon should not be parsed as lowerdir: %v", err)
	}
}

func TestPrepareOverlayRejectsInjectedLowerPathBeforeFilesystemMutation(t *testing.T) {
	containerDir := filepath.Join(t.TempDir(), "container")
	injectedLower := filepath.Join(t.TempDir(), "image,upperdir=/tmp/evil")

	_, err := PrepareOverlay(injectedLower, containerDir)
	if err == nil {
		t.Fatal("PrepareOverlay unexpectedly accepted mount-option delimiter in lower path")
	}
	if !strings.Contains(err.Error(), "unsupported mount-option delimiter") {
		t.Fatalf("unexpected error: %v", err)
	}
	if _, statErr := os.Stat(containerDir); !os.IsNotExist(statErr) {
		t.Fatalf("container directory was mutated before validation: stat err=%v", statErr)
	}
}

func TestPrepareOverlayRejectsInjectedContainerDirBeforeFilesystemMutation(t *testing.T) {
	base := t.TempDir()
	containerDir := filepath.Join(base, `container\,workdir=evil`)

	_, err := PrepareOverlay(t.TempDir(), containerDir)
	if err == nil {
		t.Fatal("PrepareOverlay unexpectedly accepted mount-option delimiter in upper/work paths")
	}
	if _, statErr := os.Stat(containerDir); !os.IsNotExist(statErr) {
		t.Fatalf("container directory was mutated before validation: stat err=%v", statErr)
	}
}

func TestPrepareOverlayMultiLayerRejectsColonInLayerBeforeFilesystemMutation(t *testing.T) {
	containerDir := filepath.Join(t.TempDir(), "container")
	layers := []string{t.TempDir(), "/tmp/top:/tmp/injected"}

	_, err := PrepareOverlayMultiLayer(layers, containerDir)
	if err == nil {
		t.Fatal("PrepareOverlayMultiLayer unexpectedly accepted ':' inside a layer path")
	}
	if !strings.Contains(err.Error(), "unsupported lowerdir delimiter") {
		t.Fatalf("unexpected error: %v", err)
	}
	if _, statErr := os.Stat(containerDir); !os.IsNotExist(statErr) {
		t.Fatalf("container directory was mutated before validation: stat err=%v", statErr)
	}
}
