package attach

import (
	"fmt"
	"io"

	"minicontainer/internal/logs"
	"minicontainer/internal/state"
)

// AttachContainer attaches stdio streams to a running container's output log stream.
func AttachContainer(st *state.Store, containerID string, in io.Reader, out io.Writer) error {
	c, err := st.Resolve(containerID)
	if err != nil {
		return fmt.Errorf("resolve container: %w", err)
	}

	if c.Status != state.StatusRunning {
		return fmt.Errorf("container %s is not running (status: %s)", c.ID, c.Status)
	}

	fmt.Fprintf(out, "You are attached to container %s. Press Ctrl+C to detach.\n", c.ID[:min(8, len(c.ID))])
	if err := logs.PrintLogs(c.ID, 0, false, out); err != nil {
		return fmt.Errorf("open container log stream: %w", err)
	}
	return nil
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
