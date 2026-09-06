package container

import (
	"crypto/sha256"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"minicontainer/internal/image"
	"minicontainer/internal/state"
)

// CommitContainer creates a new image from a container's current rootfs state.
func CommitContainer(st *state.Store, containerID, targetTag string) (result *state.Image, retErr error) {
	if st == nil {
		return nil, fmt.Errorf("state store is nil")
	}
	if targetTag == "" {
		return nil, fmt.Errorf("target tag cannot be empty")
	}

	c, err := st.Resolve(containerID)
	if err != nil {
		return nil, fmt.Errorf("resolve container: %w", err)
	}

	// Commit payload ownership uses the complete digest-sized identity. The old
	// 12-hex truncation made the destructive storage namespace only 48 bits wide.
	idBytes := sha256.Sum256([]byte(fmt.Sprintf("%s-%d", c.ID, time.Now().UnixNano())))
	imgID := fmt.Sprintf("%x", idBytes)

	lease, err := st.AcquireImageStorage()
	if err != nil {
		return nil, fmt.Errorf("acquire image storage generation: %w", err)
	}
	defer func() {
		if err := lease.Close(); err != nil {
			retErr = errors.Join(retErr, fmt.Errorf("close image storage lease: %w", err))
		}
	}()

	imagesDir := lease.Path()
	durableImagesDir := lease.DurablePath()
	imgDir := filepath.Join(imagesDir, imgID)
	rootFS := filepath.Join(durableImagesDir, imgID, "rootfs")

	// Assemble the committed image out-of-place inside the exact image-directory
	// generation pinned by Store.Open. No final image path exists until export
	// and unpack have both completed successfully.
	tmpDir, err := os.MkdirTemp(imagesDir, ".commit-"+imgID+"-")
	if err != nil {
		return nil, fmt.Errorf("create committed image staging directory: %w", err)
	}
	cleanupPending := true
	defer func() {
		if !cleanupPending {
			return
		}
		if err := os.RemoveAll(tmpDir); err != nil {
			result = nil
			retErr = errors.Join(retErr, fmt.Errorf("remove committed image staging directory %q: %w", tmpDir, err))
		}
	}()

	tmpRootFS := filepath.Join(tmpDir, "rootfs")
	if err := os.MkdirAll(tmpRootFS, 0o755); err != nil {
		return nil, fmt.Errorf("create committed rootfs staging directory: %w", err)
	}
	tarPath := filepath.Join(tmpDir, "layer.tar.gz")
	if err := image.ExportDir(c.RootFS, tarPath); err != nil {
		return nil, fmt.Errorf("export container rootfs: %w", err)
	}
	if err := image.Unpack(tarPath, tmpRootFS); err != nil {
		return nil, fmt.Errorf("unpack committed layer: %w", err)
	}

	if err := os.Rename(tmpDir, imgDir); err != nil {
		return nil, fmt.Errorf("publish committed image directory: %w", err)
	}
	cleanupPending = false
	publishedOwned := true

	if err := lease.ValidateConfiguredGeneration(); err != nil {
		boundaryErr := fmt.Errorf("validate image storage generation before commit metadata publication: %w", err)
		if cleanupErr := os.RemoveAll(imgDir); cleanupErr != nil {
			boundaryErr = errors.Join(boundaryErr, fmt.Errorf("rollback unpublished committed image %q: %w", imgDir, cleanupErr))
		}
		return nil, boundaryErr
	}

	img := &state.Image{
		ID:       imgID,
		Name:     targetTag,
		Tag:      targetTag,
		RootFS:   rootFS,
		LoadedAt: time.Now(),
	}

	if err := st.PublishImage(img); err != nil {
		saveErr := error(fmt.Errorf("save image record: %w", err))
		if publishedOwned {
			referenced, proofErr := committedImagePayloadHasReference(st, imgID, rootFS)
			switch {
			case proofErr != nil:
				saveErr = errors.Join(saveErr, fmt.Errorf("preserve newly published committed image because metadata absence is unproven: %w", proofErr))
			case referenced:
				// PublishImage may have committed the record before reporting a later
				// metadata-maintenance error. A durable reference makes deletion unsafe.
			default:
				if cleanupErr := os.RemoveAll(imgDir); cleanupErr != nil {
					saveErr = errors.Join(saveErr, fmt.Errorf("rollback committed image payload after metadata failure %q: %w", imgDir, cleanupErr))
				}
			}
		}
		return nil, saveErr
	}

	return img, nil
}

func committedImagePayloadHasReference(st *state.Store, imageID, rootFS string) (bool, error) {
	images, err := st.ListImages()
	if err != nil {
		return false, fmt.Errorf("read image metadata ownership proof: %w", err)
	}
	wantRootFS := filepath.Clean(rootFS)
	found := false
	for _, img := range images {
		if img == nil || img.ID != imageID {
			continue
		}
		if filepath.Clean(img.RootFS) != wantRootFS {
			return false, fmt.Errorf(
				"image ID %s has committed metadata for unexpected rootfs %q, want %q",
				imageID,
				img.RootFS,
				rootFS,
			)
		}
		found = true
	}
	return found, nil
}
