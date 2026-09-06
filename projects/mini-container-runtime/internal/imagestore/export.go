package imagestore

import (
	"fmt"

	"minicontainer/internal/image"
	"minicontainer/internal/state"
)

// ExportContainerRootFS exports a container's rootfs into a tarball archive.
func ExportContainerRootFS(st *state.Store, containerID, outputPath string) error {
	c, err := st.Resolve(containerID)
	if err != nil {
		return fmt.Errorf("resolve container: %w", err)
	}

	if c.RootFS == "" {
		return fmt.Errorf("container %s has no rootfs path", c.ID[:8])
	}

	return image.ExportDir(c.RootFS, outputPath)
}
