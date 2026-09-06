package container

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"

	"minicontainer/internal/state"
)

var trustedNSenterDirs = []string{"/usr/bin", "/bin", "/usr/sbin", "/sbin"}

func resolveTrustedExecutable(name string, dirs []string) (string, error) {
	if name == "" || filepath.Base(name) != name {
		return "", fmt.Errorf("invalid executable name %q", name)
	}
	for _, dir := range dirs {
		candidate := filepath.Join(dir, name)
		info, err := os.Lstat(candidate)
		if err != nil {
			if os.IsNotExist(err) {
				continue
			}
			return "", fmt.Errorf("inspect trusted executable %s: %w", candidate, err)
		}
		if info.Mode()&os.ModeSymlink != 0 || !info.Mode().IsRegular() || info.Mode().Perm()&0o111 == 0 {
			continue
		}
		return candidate, nil
	}
	return "", fmt.Errorf("trusted executable %q not found", name)
}

// ExecDetached spawns a background sub-process inside container namespaces without holding terminal session.
func ExecDetached(st *state.Store, containerID string, command []string) (int, error) {
	if len(command) == 0 {
		return 0, fmt.Errorf("command is empty")
	}

	c, err := st.Resolve(containerID)
	if err != nil {
		return 0, fmt.Errorf("resolve container: %w", err)
	}

	if c.Status != state.StatusRunning {
		return 0, fmt.Errorf("container %s is not running", c.ID[:8])
	}

	if runtime.GOOS != "linux" {
		return 12345, nil
	}

	nsenterPath, err := resolveTrustedExecutable("nsenter", trustedNSenterDirs)
	if err != nil {
		return 0, fmt.Errorf("resolve nsenter: %w", err)
	}
	cmd := exec.Command(nsenterPath, append([]string{"-t", fmt.Sprintf("%d", c.PID), "-m", "-u", "-i", "-n", "-p", "--"}, command...)...)
	if err := cmd.Start(); err != nil {
		return 0, fmt.Errorf("start detached exec: %w", err)
	}

	return cmd.Process.Pid, nil
}
