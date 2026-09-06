package container

import (
	"fmt"
	"os"
	"path/filepath"
	"time"

	"minicontainer/internal/image"
	"minicontainer/internal/state"
)

type Snapshot struct {
	Name        string    `json:"name"`
	ContainerID string    `json:"container_id"`
	CreatedAt   time.Time `json:"created_at"`
	Path        string    `json:"path"`
}

func SnapshotsDir(containerID string) string {
	return filepath.Join(state.DefaultDir(), "containers", containerID, "snapshots")
}

// CreateSnapshot archives the container's rootfs into a named snapshot tarball.
func CreateSnapshot(st *state.Store, containerID, snapName string) (*Snapshot, error) {
	c, err := st.Resolve(containerID)
	if err != nil {
		return nil, fmt.Errorf("resolve container: %w", err)
	}

	sDir := SnapshotsDir(c.ID)
	if err := os.MkdirAll(sDir, 0755); err != nil {
		return nil, err
	}

	outTar := filepath.Join(sDir, snapName+".tar.gz")
	if err := image.ExportDir(c.RootFS, outTar); err != nil {
		return nil, fmt.Errorf("export rootfs snapshot: %w", err)
	}

	snap := &Snapshot{
		Name:        snapName,
		ContainerID: c.ID,
		CreatedAt:   time.Now(),
		Path:        outTar,
	}

	return snap, nil
}

// RestoreSnapshot unpacks a named snapshot back into the container's rootfs.
func RestoreSnapshot(st *state.Store, containerID, snapName string) error {
	c, err := st.Resolve(containerID)
	if err != nil {
		return fmt.Errorf("resolve container: %w", err)
	}

	snapTar := filepath.Join(SnapshotsDir(c.ID), snapName+".tar.gz")
	if _, err := os.Stat(snapTar); err != nil {
		return fmt.Errorf("snapshot %q not found for container %s", snapName, c.ID[:8])
	}

	return image.Unpack(snapTar, c.RootFS)
}
