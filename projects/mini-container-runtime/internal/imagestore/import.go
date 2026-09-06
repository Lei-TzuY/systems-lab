package imagestore

import (
	"crypto/sha256"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"time"

	"minicontainer/internal/image"
	"minicontainer/internal/state"
)

type importRemoveAllFunc func(path string) error

// ImportRawRootFS unpacks a raw tarball into imagestore and tags it.
func ImportRawRootFS(st *state.Store, tarPath, imageTag string) (*state.Image, error) {
	return importRawRootFSWithCleanup(st, tarPath, imageTag, os.RemoveAll)
}

func importRawRootFSWithCleanup(st *state.Store, tarPath, imageTag string, removeAll importRemoveAllFunc) (result *state.Image, retErr error) {
	if st == nil {
		return nil, fmt.Errorf("state store is nil")
	}
	if imageTag == "" {
		return nil, fmt.Errorf("image tag cannot be empty")
	}
	if removeAll == nil {
		return nil, fmt.Errorf("image staging cleanup operation is nil")
	}

	f, err := os.Open(tarPath)
	if err != nil {
		return nil, fmt.Errorf("open tarball: %w", err)
	}
	h := sha256.New()
	size, err := io.Copy(h, f)
	closeErr := f.Close()
	if err != nil {
		return nil, fmt.Errorf("hash tarball: %w", err)
	}
	if closeErr != nil {
		return nil, fmt.Errorf("close tarball after hashing: %w", closeErr)
	}

	lease, err := st.AcquireImageStorage()
	if err != nil {
		return nil, fmt.Errorf("acquire image storage generation: %w", err)
	}
	defer func() {
		if err := lease.Close(); err != nil {
			retErr = errors.Join(retErr, fmt.Errorf("close image storage lease: %w", err))
		}
	}()

	sum, err := rawRootFSContentID(h.Sum(nil))
	if err != nil {
		return nil, fmt.Errorf("derive raw rootfs content identity: %w", err)
	}
	imagesDir := lease.Path()
	durableImagesDir := lease.DurablePath()
	imgDir := filepath.Join(imagesDir, sum)
	rootFS := filepath.Join(durableImagesDir, sum, "rootfs")

	// Build the rootfs out-of-place inside the exact image-directory generation
	// pinned by Store.Open. A replaced configured state pathname can therefore
	// never redirect extraction or publication into another filesystem tree.
	tmpDir, err := os.MkdirTemp(imagesDir, ".import-"+sum+"-")
	if err != nil {
		return nil, fmt.Errorf("create temporary image directory: %w", err)
	}
	cleanupPending := true
	cleanupStaging := func(context string) error {
		if !cleanupPending {
			return nil
		}
		if err := removeAll(tmpDir); err != nil {
			return fmt.Errorf("%s %q: %w", context, tmpDir, err)
		}
		cleanupPending = false
		return nil
	}
	defer func() {
		if !cleanupPending {
			return
		}
		if err := cleanupStaging("remove temporary image directory"); err != nil {
			result = nil
			retErr = errors.Join(retErr, err)
		}
	}()

	tmpRootFS := filepath.Join(tmpDir, "rootfs")
	if err := os.MkdirAll(tmpRootFS, 0755); err != nil {
		return nil, fmt.Errorf("create temporary rootfs: %w", err)
	}
	if err := image.Unpack(tarPath, tmpRootFS); err != nil {
		return nil, fmt.Errorf("unpack rootfs: %w", err)
	}

	publishedOwned := false
	if err := os.Rename(tmpDir, imgDir); err != nil {
		if proofErr := verifyReusableRawRootFSPayload(st, imgDir, rootFS, sum); proofErr != nil {
			return nil, errors.Join(
				fmt.Errorf("publish image rootfs: %w", err),
				fmt.Errorf("refuse unproven existing image payload: %w", proofErr),
			)
		}
		// A previously committed exact full-digest payload may be reused. Before
		// writing another tag record, the private staging directory must actually
		// be gone; otherwise a successful import would leave durable garbage.
		if cleanupErr := cleanupStaging("discard duplicate import staging"); cleanupErr != nil {
			return nil, cleanupErr
		}
	} else {
		// tmpDir has moved to imgDir, so there is no staging pathname left for
		// this call to clean up. This call exclusively owns the published path
		// until its image metadata is committed.
		cleanupPending = false
		publishedOwned = true
	}

	// RootFS metadata uses the configured durable path, not /proc/self/fd. Prove
	// immediately before metadata publication that the configured root/images
	// path still names this exact leased generation. If the pathname changed
	// while extraction was running, remove only content published by this call.
	if err := lease.ValidateConfiguredGeneration(); err != nil {
		boundaryErr := fmt.Errorf("validate image storage generation before metadata publication: %w", err)
		if publishedOwned {
			if cleanupErr := removeAll(imgDir); cleanupErr != nil {
				boundaryErr = errors.Join(boundaryErr, fmt.Errorf("rollback unpublished image rootfs %q: %w", imgDir, cleanupErr))
			}
		}
		return nil, boundaryErr
	}

	imgRec := &state.Image{
		ID:       sum,
		Name:     imageTag,
		Tag:      imageTag,
		RootFS:   rootFS,
		Size:     size,
		LoadedAt: time.Now(),
	}

	if err := st.PublishImage(imgRec); err != nil {
		saveErr := error(fmt.Errorf("save image record: %w", err))
		if publishedOwned {
			referenced, proofErr := rawRootFSPayloadHasCommittedReference(st, rootFS, sum)
			switch {
			case proofErr != nil:
				saveErr = errors.Join(saveErr, fmt.Errorf("preserve newly published image payload because metadata absence is unproven: %w", proofErr))
			case referenced:
				// PublishImage can report a post-commit maintenance failure. A
				// durable reference proves that deleting the payload would create
				// dangling metadata.
			default:
				if cleanupErr := removeAll(imgDir); cleanupErr != nil {
					saveErr = errors.Join(saveErr, fmt.Errorf("rollback image payload after metadata failure %q: %w", imgDir, cleanupErr))
				}
			}
		}
		return nil, saveErr
	}

	return imgRec, nil
}
