package network

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
)

var trustedHostToolDirs = []string{"/usr/bin", "/bin", "/usr/sbin", "/sbin"}

func resolveTrustedHostTool(name string, dirs []string) (string, error) {
	if name == "" || filepath.Base(name) != name {
		return "", fmt.Errorf("invalid host tool name %q", name)
	}
	for _, dir := range dirs {
		candidate := filepath.Join(dir, name)
		info, err := os.Lstat(candidate)
		if err != nil {
			if os.IsNotExist(err) {
				continue
			}
			return "", fmt.Errorf("inspect trusted host tool %s: %w", candidate, err)
		}
		if info.Mode()&os.ModeSymlink != 0 || !info.Mode().IsRegular() || info.Mode().Perm()&0o111 == 0 {
			continue
		}
		return candidate, nil
	}
	return "", fmt.Errorf("trusted host tool %q not found", name)
}

func runTrustedHostTool(name string, args ...string) ([]byte, error) {
	path, err := resolveTrustedHostTool(name, trustedHostToolDirs)
	if err != nil {
		return nil, err
	}
	return exec.Command(path, args...).CombinedOutput()
}
