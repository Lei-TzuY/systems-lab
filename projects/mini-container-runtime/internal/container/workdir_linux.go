//go:build linux

package container

import (
	"fmt"
	"os"
	"syscall"
)

type mkdirAllCall func(path string, perm os.FileMode) error
type chdirCall func(path string) error

func enterWorkDir(workDir string) error {
	return enterWorkDirWith(workDir, os.MkdirAll, syscall.Chdir)
}

func enterWorkDirWith(workDir string, mkdirAll mkdirAllCall, chdir chdirCall) error {
	if workDir == "" || workDir == "/" {
		return nil
	}
	if mkdirAll == nil || chdir == nil {
		return fmt.Errorf("workdir filesystem operation is nil")
	}
	if err := mkdirAll(workDir, 0o755); err != nil {
		return fmt.Errorf("create workdir %s: %w", workDir, err)
	}
	if err := chdir(workDir); err != nil {
		return fmt.Errorf("chdir workdir %s: %w", workDir, err)
	}
	return nil
}
