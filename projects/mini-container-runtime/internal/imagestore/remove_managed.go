package imagestore

import (
	"fmt"
	"path/filepath"
	"reflect"
	"strings"

	"minicontainer/internal/state"
)

type managedImageRootFSRemoval interface {
	Remove() error
	Close() error
}

func prepareManagedImageRootFSRemoval(lease *state.ImageStorageLease, img *state.Image) (managedImageRootFSRemoval, bool, error) {
	if lease == nil {
		return nil, false, fmt.Errorf("image storage lease is nil")
	}
	if img == nil {
		return nil, false, fmt.Errorf("image state is nil")
	}
	if img.RootFS == "" {
		return nil, false, nil
	}

	durableImages := filepath.Clean(lease.DurablePath())
	rootFS := filepath.Clean(img.RootFS)
	rel, err := filepath.Rel(durableImages, rootFS)
	if err != nil {
		// Paths on another volume (notably on Windows) are external to managed
		// image storage and retain the historical external-rootfs behavior.
		return nil, false, nil
	}
	parentEscape := ".." + string(filepath.Separator)
	if rel == ".." || strings.HasPrefix(rel, parentEscape) {
		return nil, false, nil
	}

	// Once metadata points anywhere inside the managed image directory, its
	// shape and ownership identity must be exact. Malformed managed metadata
	// must not be downgraded to a generic pathname deletion.
	if img.ID == "" || img.ID == "." || img.ID == ".." || filepath.Base(img.ID) != img.ID || strings.ContainsAny(img.ID, "/\\\x00") {
		return nil, true, fmt.Errorf("managed image has unsafe payload ID %q", img.ID)
	}
	parts := strings.Split(rel, string(filepath.Separator))
	if len(parts) != 2 || parts[0] != img.ID || parts[1] != "rootfs" {
		return nil, true, fmt.Errorf(
			"managed image rootfs %q does not match expected %q",
			img.RootFS,
			filepath.Join(durableImages, img.ID, "rootfs"),
		)
	}

	removal, err := pinManagedImageRootFS(lease.Path(), img.ID)
	if err != nil {
		return nil, true, err
	}
	return removal, true, nil
}

func imageRootFSLooksManaged(configuredImages, rootFS string) bool {
	if rootFS == "" {
		return false
	}
	rel, err := filepath.Rel(filepath.Clean(configuredImages), filepath.Clean(rootFS))
	if err != nil {
		return false
	}
	parentEscape := ".." + string(filepath.Separator)
	return rel != ".." && !strings.HasPrefix(rel, parentEscape)
}

func imageSnapshotContains(images []*state.Image, target *state.Image) bool {
	if target == nil {
		return false
	}
	for _, img := range images {
		if reflect.DeepEqual(img, target) {
			return true
		}
	}
	return false
}

func validateImageAliasRootFSConsistency(images []*state.Image) error {
	byID := make(map[string]string)
	for _, img := range images {
		if img == nil || img.ID == "" {
			continue
		}
		rootFS := filepath.Clean(img.RootFS)
		if previous, ok := byID[img.ID]; ok && previous != rootFS {
			return fmt.Errorf(
				"inconsistent image aliases for ID %s reference rootfs %q and %q",
				img.ID,
				previous,
				rootFS,
			)
		}
		byID[img.ID] = rootFS
	}
	return nil
}

func imageRootFSReferenced(images []*state.Image, rootFS string) bool {
	want := filepath.Clean(rootFS)
	for _, img := range images {
		if img != nil && filepath.Clean(img.RootFS) == want {
			return true
		}
	}
	return false
}
