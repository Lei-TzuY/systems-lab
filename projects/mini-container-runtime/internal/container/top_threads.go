package container

import (
	"fmt"
	"os"
	"path/filepath"

	"minicontainer/internal/state"
)

type ThreadInfo struct {
	TID  int    `json:"tid"`
	Name string `json:"name"`
}

// GetContainerThreads inspects thread-level task details for a running container.
func GetContainerThreads(st *state.Store, containerID string) ([]ThreadInfo, error) {
	c, err := st.Resolve(containerID)
	if err != nil {
		return nil, fmt.Errorf("resolve container: %w", err)
	}

	if c.Status != state.StatusRunning || c.PID <= 0 {
		return nil, fmt.Errorf("container %s is not running", c.ID[:8])
	}

	taskDir := fmt.Sprintf("/proc/%d/task", c.PID)
	entries, err := os.ReadDir(taskDir)
	if err != nil {
		return []ThreadInfo{{TID: c.PID, Name: "main"}}, nil
	}

	var threads []ThreadInfo
	for _, entry := range entries {
		var tid int
		if _, err := fmt.Sscanf(entry.Name(), "%d", &tid); err == nil {
			commFile := filepath.Join(taskDir, entry.Name(), "comm")
			comm, _ := os.ReadFile(commFile)
			name := "thread"
			if len(comm) > 0 {
				name = string(comm)
			}
			threads = append(threads, ThreadInfo{TID: tid, Name: name})
		}
	}

	return threads, nil
}
