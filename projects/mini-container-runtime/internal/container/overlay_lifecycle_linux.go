//go:build linux

package container

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

const (
	overlayWorkDirEnv       = "MINICONTAINER_OVERLAY_WORKDIR"
	overlayWorkDirPrefix    = "minicontainer-overlay-"
	runtimeControlEnvPrefix = "MINICONTAINER_"
)

type overlayMkdirTemp func(dir, pattern string) (string, error)
type overlayRemoveAll func(path string) error

// createParentOverlayWorkDir allocates overlay storage before the child starts,
// so the parent has an authoritative path to remove after the child has exited.
// Overlay allocation is a runtime-control operation: restarting the payload
// cannot repair a runtime that cannot allocate its isolation storage.
func createParentOverlayWorkDir(enabled bool, mkdirTemp overlayMkdirTemp) (string, error) {
	if !enabled {
		return "", nil
	}
	if mkdirTemp == nil {
		return "", &runtimeSetupError{err: fmt.Errorf("create overlay workdir: mkdir operation is nil")}
	}
	dir, err := mkdirTemp("", overlayWorkDirPrefix+"*")
	if err != nil {
		return "", &runtimeSetupError{err: fmt.Errorf("create overlay workdir: %w", err)}
	}
	if dir == "" {
		return "", &runtimeSetupError{err: fmt.Errorf("create overlay workdir: empty path returned")}
	}
	return dir, nil
}

// appendOverlayWorkDirEnv passes the parent-owned path only to the re-exec
// child. ContainerInit consumes and removes this private variable before the
// payload environment is built.
func appendOverlayWorkDirEnv(env []string, dir string) []string {
	if dir == "" {
		return env
	}
	return append(env, overlayWorkDirEnv+"="+dir)
}

// clearRuntimeControlEnvironment removes ambient bootstrap/control variables
// inherited by the runtime re-exec. Explicit container environment entries are
// carried in Config.Env and appended only when the payload environment is built,
// so this does not strip user-requested payload variables.
func clearRuntimeControlEnvironment() error {
	for _, entry := range os.Environ() {
		key, _, ok := strings.Cut(entry, "=")
		if !ok || !strings.HasPrefix(key, runtimeControlEnvPrefix) {
			continue
		}
		if err := os.Unsetenv(key); err != nil {
			return fmt.Errorf("clear runtime control environment %q: %w", key, err)
		}
	}
	return nil
}

// consumeOverlayWorkDir reads the parent-issued overlay path and then clears
// every ambient MINICONTAINER_* control variable before payload setup proceeds.
// The path is validated before PrepareOverlay can create children beneath it,
// so a forged environment value cannot redirect overlay setup into an arbitrary
// host directory.
func consumeOverlayWorkDir(enabled bool) (string, error) {
	dir := os.Getenv(overlayWorkDirEnv)
	if err := clearRuntimeControlEnvironment(); err != nil {
		return "", err
	}
	if !enabled {
		return "", nil
	}
	if dir == "" {
		return "", fmt.Errorf("runtime parent did not provide an overlay workdir")
	}
	if !filepath.IsAbs(dir) {
		return "", fmt.Errorf("runtime overlay workdir %q is not absolute", dir)
	}
	if !strings.HasPrefix(filepath.Base(dir), overlayWorkDirPrefix) {
		return "", fmt.Errorf("runtime overlay workdir %q has an unexpected name", dir)
	}

	info, err := os.Lstat(dir)
	if err != nil {
		return "", fmt.Errorf("inspect runtime overlay workdir %q: %w", dir, err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return "", fmt.Errorf("runtime overlay workdir %q is not a real directory", dir)
	}
	if info.Mode().Perm()&0o077 != 0 {
		return "", fmt.Errorf("runtime overlay workdir %q is not private: mode %04o", dir, info.Mode().Perm())
	}
	return dir, nil
}

// finishOverlayWorkDir removes the exact path allocated by the parent. It is
// designed for a named-return defer in runOnce so every post-allocation return
// path is covered. Cleanup failures are runtime-control failures and are joined
// with an existing payload/setup error rather than replacing it.
func finishOverlayWorkDir(resultErr error, dir string, removeAll overlayRemoveAll) error {
	if dir == "" {
		return resultErr
	}
	if removeAll == nil {
		cleanupErr := &runtimeSetupError{err: fmt.Errorf("cleanup overlay workdir %q: remove operation is nil", dir)}
		return errors.Join(resultErr, cleanupErr)
	}
	if err := removeAll(dir); err != nil {
		cleanupErr := &runtimeSetupError{err: fmt.Errorf("cleanup overlay workdir %q: %w", dir, err)}
		return errors.Join(resultErr, cleanupErr)
	}
	return resultErr
}
