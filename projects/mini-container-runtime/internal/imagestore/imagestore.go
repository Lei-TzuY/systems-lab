package imagestore

import (
	"crypto/rand"
	"encoding/hex"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"time"

	"minicontainer/internal/state"
)

// GenerateImageID returns a random 12-hex-character ID for an image.
func GenerateImageID() string {
	b := make([]byte, 6)
	if _, err := rand.Read(b); err != nil {
		return fmt.Sprintf("%012x", time.Now().UnixNano())
	}
	return hex.EncodeToString(b)
}

type walkDirFunc func(string, fs.WalkDirFunc) error

// CalculateDirSize recursively calculates total bytes of files inside path.
// A partial traversal is not a trustworthy size, so any walk or metadata error
// returns zero together with the underlying error.
func CalculateDirSize(root string) (int64, error) {
	return calculateDirSizeWithWalk(root, filepath.WalkDir)
}

func calculateDirSizeWithWalk(root string, walk walkDirFunc) (int64, error) {
	var total int64
	err := walk(root, func(current string, d fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return fmt.Errorf("walk %q: %w", current, walkErr)
		}
		if d.IsDir() {
			return nil
		}
		info, err := d.Info()
		if err != nil {
			return fmt.Errorf("inspect %q while calculating directory size: %w", current, err)
		}
		total += info.Size()
		return nil
	})
	if err != nil {
		return 0, err
	}
	return total, nil
}

// ParseRepositoryTag splits "ubuntu:22.04" into ("ubuntu", "22.04")
func ParseRepositoryTag(imageName string) (string, string) {
	if strings.Contains(imageName, ":") {
		parts := strings.SplitN(imageName, ":", 2)
		return parts[0], parts[1]
	}
	return imageName, "latest"
}

// TagImage creates an alias tag for an existing image.
func TagImage(st *state.Store, source, target string) (*state.Image, error) {
	if st == nil {
		return nil, fmt.Errorf("state store is nil")
	}
	img, err := st.GetImage(source)
	if err != nil {
		return nil, fmt.Errorf("source image %q not found: %w", source, err)
	}

	repo, tag := ParseRepositoryTag(target)
	newImg := *img
	newImg.Name = target
	newImg.Repository = repo
	newImg.Tag = tag

	if err := st.PublishImageIfSourceMatch(source, img, &newImg); err != nil {
		return nil, fmt.Errorf("save tagged image %q: %w", target, err)
	}
	return &newImg, nil
}

// RemoveImage removes image metadata and optionally cleans up the rootfs folder if no other tags reference it.
func RemoveImage(st *state.Store, nameOrID string, removeRootFS bool) (*state.Image, error) {
	return removeImage(st, nameOrID, nil, removeRootFS)
}

// RemoveImageIfMatch removes an image only while the selector still resolves to
// the exact durable snapshot previously observed by the caller. It is intended
// for prune/reconcile loops that must not delete a newer tag generation that
// replaced their list snapshot before destructive cleanup begins.
func RemoveImageIfMatch(st *state.Store, nameOrID string, expected *state.Image, removeRootFS bool) (*state.Image, error) {
	if expected == nil {
		return nil, fmt.Errorf("expected image state is nil")
	}
	return removeImage(st, nameOrID, expected, removeRootFS)
}

func removeImage(st *state.Store, nameOrID string, expected *state.Image, removeRootFS bool) (result *state.Image, retErr error) {
	if st == nil {
		return nil, fmt.Errorf("state store is nil")
	}
	if !removeRootFS {
		if expected != nil {
			return st.DeleteImageIfMatch(nameOrID, expected)
		}
		return st.DeleteImage(nameOrID)
	}

	// Rootfs removal is destructive. Prove that the complete metadata index is
	// readable and internally consistent before pinning or mutating anything.
	snapshot, err := st.ListImages()
	if err != nil {
		return nil, fmt.Errorf("preflight image metadata before removal: %w", err)
	}
	if err := validateImageAliasRootFSConsistency(snapshot); err != nil {
		return nil, fmt.Errorf("preflight image alias ownership before removal: %w", err)
	}
	img, err := st.GetImage(nameOrID)
	if err != nil {
		return nil, err
	}
	if expected != nil && !reflect.DeepEqual(img, expected) {
		return nil, fmt.Errorf("image %q changed after prune snapshot", nameOrID)
	}
	if !imageSnapshotContains(snapshot, img) {
		return nil, fmt.Errorf("image %q changed during destructive preflight", nameOrID)
	}

	var lease *state.ImageStorageLease
	var pinned managedImageRootFSRemoval
	if img.RootFS != "" && imageRootFSLooksManaged(filepath.Join(st.Dir(), "images"), img.RootFS) {
		lease, err = st.AcquireImageStorage()
		if err != nil {
			return nil, fmt.Errorf("acquire managed image storage for removal: %w", err)
		}
		defer func() {
			if err := lease.Close(); err != nil {
				retErr = errors.Join(retErr, fmt.Errorf("close managed image storage lease: %w", err))
			}
		}()

		var managed bool
		pinned, managed, err = prepareManagedImageRootFSRemoval(lease, img)
		if err != nil {
			return nil, fmt.Errorf("pin managed image rootfs before metadata removal: %w", err)
		}
		if !managed || pinned == nil {
			return nil, fmt.Errorf("managed image rootfs %q could not be pinned", img.RootFS)
		}
		defer func() {
			if err := pinned.Close(); err != nil {
				retErr = errors.Join(retErr, fmt.Errorf("close pinned managed image rootfs: %w", err))
			}
		}()

		cleanup := state.ImageCleanup{ID: img.ID, RootFS: filepath.Clean(img.RootFS)}
		removed, armed, err := st.DeleteImageIfMatchWithCleanup(nameOrID, img, cleanup)
		if err != nil {
			return nil, err
		}
		result = removed
		if !armed {
			// Another durable alias still owns the payload. The state transaction
			// removed only this metadata record and intentionally armed no cleanup.
			return result, nil
		}
		if err := pinned.Remove(); err != nil {
			return result, fmt.Errorf("remove managed image rootfs %q with cleanup ownership retained: %w", removed.RootFS, err)
		}
		if _, err := st.ClearImageCleanupIfMatch(cleanup); err != nil {
			return result, fmt.Errorf("clear managed image cleanup ownership after payload removal: %w", err)
		}
		return result, nil
	}

	removed, err := st.DeleteImageIfMatch(nameOrID, img)
	if err != nil {
		return nil, err
	}
	result = removed
	if removed.RootFS == "" {
		return result, nil
	}

	// External/custom rootfs paths preserve historical behavior. They are not
	// eligible for automatic crash recovery because the Store cannot prove a
	// stable filesystem generation for an arbitrary host path.
	remaining, err := st.ListImages()
	if err != nil {
		return result, fmt.Errorf("verify image rootfs references after metadata removal: %w", err)
	}
	if err := validateImageAliasRootFSConsistency(remaining); err != nil {
		return result, fmt.Errorf("verify image alias ownership after metadata removal: %w", err)
	}
	if imageRootFSReferenced(remaining, removed.RootFS) {
		return result, nil
	}
	if err := os.RemoveAll(removed.RootFS); err != nil {
		return result, fmt.Errorf("remove image rootfs %q: %w", removed.RootFS, err)
	}
	return result, nil
}
