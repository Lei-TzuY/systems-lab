//go:build linux

package container

import (
	"fmt"
	"os"
	"strconv"

	volumestore "minicontainer/internal/volume"
)

// resolveVolumeMountSource preserves ordinary host-bind semantics, but recovers
// named-volume provenance from the managed data-path layout and reopens that
// source through the volume store's pinned descriptor chain.
func resolveVolumeMountSource(hostPath string) (string, *os.File, error) {
	name, managed := volumestore.ManagedNameFromDataPath(hostPath)
	if !managed {
		return hostPath, nil, nil
	}
	file, err := volumestore.OpenPinnedData(name)
	if err != nil {
		return "", nil, fmt.Errorf("open managed volume %q source: %w", name, err)
	}
	return "/proc/self/fd/" + strconv.Itoa(int(file.Fd())), file, nil
}
