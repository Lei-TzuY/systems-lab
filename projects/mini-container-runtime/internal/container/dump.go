package container

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"time"

	"minicontainer/internal/state"
)

type DumpInfo struct {
	ContainerID string    `json:"container_id"`
	PID         int       `json:"pid"`
	Command     []string  `json:"command"`
	Status      string    `json:"status"`
	Timestamp   time.Time `json:"timestamp"`
	OS          string    `json:"os"`
	ProcMaps    string    `json:"proc_maps,omitempty"`
}

// DumpContainerMemory captures container process state into a dump artifact file.
func DumpContainerMemory(st *state.Store, containerID string, outPath string) (*DumpInfo, error) {
	c, err := st.Resolve(containerID)
	if err != nil {
		return nil, fmt.Errorf("resolve container: %w", err)
	}

	info := &DumpInfo{
		ContainerID: c.ID,
		PID:         c.PID,
		Command:     c.Command,
		Status:      string(c.Status),
		Timestamp:   time.Now(),
		OS:          runtime.GOOS,
	}

	if runtime.GOOS == "linux" && c.PID > 0 {
		mapsPath := fmt.Sprintf("/proc/%d/maps", c.PID)
		content, err := os.ReadFile(mapsPath)
		if err == nil {
			info.ProcMaps = string(content)
		}
	}

	if outPath != "" {
		_ = os.MkdirAll(filepath.Dir(outPath), 0755)
		raw, err := json.MarshalIndent(info, "", "  ")
		if err != nil {
			return nil, fmt.Errorf("marshal dump info: %w", err)
		}
		if err := os.WriteFile(outPath, raw, 0644); err != nil {
			return nil, fmt.Errorf("write dump file: %w", err)
		}
	}

	return info, nil
}
