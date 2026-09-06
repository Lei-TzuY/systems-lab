package imagestore

import (
	"errors"
	"fmt"

	"minicontainer/internal/state"
)

func recoverPendingManagedImageCleanups(st *state.Store) error {
	if st == nil {
		return fmt.Errorf("state store is nil")
	}
	cleanups, err := st.ListImageCleanups()
	if err != nil {
		return fmt.Errorf("list pending image cleanup ownership: %w", err)
	}
	for _, cleanup := range cleanups {
		if err := recoverOneManagedImageCleanup(st, cleanup); err != nil {
			return err
		}
	}
	return nil
}

func recoverOneManagedImageCleanup(st *state.Store, cleanup state.ImageCleanup) (retErr error) {
	// A sidecar may have become durable immediately before a crash, while the
	// metadata it protects never got unlinked. Prove and retire that stale token
	// atomically under the state lock; a separate ListImages/Clear pair would
	// allow metadata to change between the proof and token removal.
	referenced, err := st.ClearImageCleanupIfReferenced(cleanup)
	if err != nil {
		return fmt.Errorf("prove pending image cleanup reference for %s: %w", cleanup.ID, err)
	}
	if referenced {
		return nil
	}

	lease, err := st.AcquireImageStorage()
	if err != nil {
		return fmt.Errorf("acquire image storage while recovering cleanup for %s: %w", cleanup.ID, err)
	}
	defer func() {
		if err := lease.Close(); err != nil {
			retErr = errors.Join(retErr, fmt.Errorf("close image storage recovery lease: %w", err))
		}
	}()

	probe := &state.Image{ID: cleanup.ID, RootFS: cleanup.RootFS}
	pinned, managed, err := prepareManagedImageRootFSRemoval(lease, probe)
	if err != nil {
		return fmt.Errorf("pin pending image cleanup for %s: %w", cleanup.ID, err)
	}
	if !managed || pinned == nil {
		return fmt.Errorf("pending image cleanup for %s is not a managed rootfs", cleanup.ID)
	}
	defer func() {
		if err := pinned.Close(); err != nil {
			retErr = errors.Join(retErr, fmt.Errorf("close pinned cleanup recovery rootfs: %w", err))
		}
	}()

	if err := pinned.Remove(); err != nil {
		return fmt.Errorf("recover pending managed image rootfs %q: %w", cleanup.RootFS, err)
	}
	if _, err := st.ClearImageCleanupIfMatch(cleanup); err != nil {
		return fmt.Errorf("clear recovered image cleanup ownership for %s: %w", cleanup.ID, err)
	}
	return nil
}
