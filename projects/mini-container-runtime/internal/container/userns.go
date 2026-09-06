package container

import (
	"fmt"
	"os"
	"runtime"
	"strings"
)

type IDMap struct {
	ContainerID int
	HostID      int
	Count       int
}

// FormatIDMap converts IDMap slices to Linux user namespace mapping string format (e.g. "0 100000 65536").
func FormatIDMap(maps []IDMap) string {
	var lines []string
	for _, m := range maps {
		lines = append(lines, fmt.Sprintf("%d %d %d", m.ContainerID, m.HostID, m.Count))
	}
	return strings.Join(lines, "\n")
}

// ApplyUserNSMappings writes uid_map and gid_map for unprivileged container PID.
func ApplyUserNSMappings(pid int, uidMaps []IDMap, gidMaps []IDMap) error {
	if runtime.GOOS != "linux" || pid <= 0 {
		return nil
	}

	if len(uidMaps) > 0 {
		uidPath := fmt.Sprintf("/proc/%d/uid_map", pid)
		if err := os.WriteFile(uidPath, []byte(FormatIDMap(uidMaps)+"\n"), 0644); err != nil {
			return fmt.Errorf("write uid_map: %w", err)
		}
	}

	if len(gidMaps) > 0 {
		gidPath := fmt.Sprintf("/proc/%d/gid_map", pid)
		if err := os.WriteFile(gidPath, []byte(FormatIDMap(gidMaps)+"\n"), 0644); err != nil {
			return fmt.Errorf("write gid_map: %w", err)
		}
	}

	return nil
}
