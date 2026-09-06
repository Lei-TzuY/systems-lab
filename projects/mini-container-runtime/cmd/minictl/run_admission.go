package main

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"minicontainer/internal/container"
	"minicontainer/internal/state"
)

type runAdmissionDeps struct {
	openStore func() (*state.Store, error)
	newID     func() (string, error)
	now       func() time.Time
}

type runRootFSAdmissionDeps struct {
	abs          func(string) (string, error)
	stat         func(string) (os.FileInfo, error)
	evalSymlinks func(string) (string, error)
}

func prepareManagedRunState(cfg *container.Config) (*state.Store, *state.Container, error) {
	return prepareManagedRunStateWith(cfg, runAdmissionDeps{
		openStore: openStore,
		newID:     state.NewID,
		now:       time.Now,
	})
}

func prepareManagedRunStateWith(cfg *container.Config, deps runAdmissionDeps) (*state.Store, *state.Container, error) {
	if cfg == nil {
		return nil, nil, fmt.Errorf("run config is nil")
	}
	if cfg.ContainerID != "" {
		return nil, nil, fmt.Errorf("run config already has container ID %q", cfg.ContainerID)
	}
	rootfs, err := normalizeRunAdmissionRootFS(cfg.RootFS)
	if err != nil {
		return nil, nil, err
	}
	rootfsIdentity, err := os.Stat(rootfs)
	if err != nil {
		return nil, nil, fmt.Errorf("capture admitted run rootfs identity %q: %w", rootfs, err)
	}
	if !rootfsIdentity.IsDir() {
		return nil, nil, fmt.Errorf("admitted run rootfs %q is not a directory", rootfs)
	}
	if deps.openStore == nil || deps.newID == nil || deps.now == nil {
		return nil, nil, fmt.Errorf("run admission dependencies are incomplete")
	}

	st, err := deps.openStore()
	if err != nil {
		return nil, nil, fmt.Errorf("open state store: %w", err)
	}
	if st == nil {
		return nil, nil, fmt.Errorf("open state store returned nil store")
	}
	fail := func(cause error) (*state.Store, *state.Container, error) {
		if closeErr := st.Close(); closeErr != nil {
			cause = errors.Join(cause, fmt.Errorf("close state store after run admission failure: %w", closeErr))
		}
		return nil, nil, cause
	}

	runtimeEnv, err := imageEnvironmentForRootFS(st, rootfs, cfg.Env)
	if err != nil {
		return fail(fmt.Errorf("resolve image environment for run: %w", err))
	}
	runtimeWorkDir, err := imageWorkingDirForRootFS(st, rootfs, cfg.WorkDir)
	if err != nil {
		return fail(fmt.Errorf("resolve image WorkingDir for run: %w", err))
	}
	runtimeCommand, err := imageCommandForRootFS(st, rootfs, cfg.Command)
	if err != nil {
		return fail(fmt.Errorf("resolve image command for run: %w", err))
	}

	id, err := deps.newID()
	if err != nil {
		return fail(fmt.Errorf("generate container ID: %w", err))
	}

	rec := &state.Container{
		ID:        id,
		Status:    state.StatusCreated,
		RootFS:    rootfs,
		Command:   append([]string(nil), runtimeCommand...),
		Hostname:  cfg.Hostname,
		CreatedAt: deps.now(),
		Env:       append([]string(nil), runtimeEnv...),
	}
	if err := st.Save(rec); err != nil {
		return fail(fmt.Errorf("persist created state for container %s: %w", id, err))
	}

	stopSignal, err := imageStopSignalForRootFS(st, rootfs)
	if err != nil {
		if rollbackErr := st.Delete(id); rollbackErr != nil {
			err = errors.Join(err, fmt.Errorf("rollback created container state: %w", rollbackErr))
		}
		return fail(fmt.Errorf("resolve image stop signal for container %s: %w", id, err))
	}
	if stopSignal != "" {
		if err := st.SaveContainerStopSignal(id, stopSignal); err != nil {
			if rollbackErr := st.Delete(id); rollbackErr != nil {
				err = errors.Join(err, fmt.Errorf("rollback created container state: %w", rollbackErr))
			}
			return fail(fmt.Errorf("persist image stop signal for container %s: %w", id, err))
		}
	}

	// Publishing the normalized rootfs, its admitted filesystem identity,
	// resolved runtime environment/workdir/command, and ID is the admission
	// commit point. An uncertain state write that returned an error must never
	// mutate the runtime config even if a filesystem entry happened to become
	// visible before that error.
	cfg.RootFS = rootfs
	cfg.RootFSIdentity = rootfsIdentity
	cfg.Env = runtimeEnv
	cfg.WorkDir = runtimeWorkDir
	cfg.Command = runtimeCommand
	cfg.ContainerID = id
	return st, rec, nil
}

func normalizeRunAdmissionRootFS(rootfs string) (string, error) {
	return normalizeRunAdmissionRootFSWith(rootfs, runRootFSAdmissionDeps{
		abs:          filepath.Abs,
		stat:         os.Stat,
		evalSymlinks: filepath.EvalSymlinks,
	})
}

func normalizeRunAdmissionRootFSWith(rootfs string, deps runRootFSAdmissionDeps) (string, error) {
	if rootfs == "" {
		return "", fmt.Errorf("run config rootfs is empty")
	}
	if deps.abs == nil || deps.stat == nil || deps.evalSymlinks == nil {
		return "", fmt.Errorf("run rootfs admission dependencies are incomplete")
	}

	abs, err := deps.abs(rootfs)
	if err != nil {
		return "", fmt.Errorf("resolve run rootfs %q: %w", rootfs, err)
	}
	abs = filepath.Clean(abs)
	before, err := deps.stat(abs)
	if err != nil {
		return "", fmt.Errorf("stat run rootfs %q: %w", abs, err)
	}
	if !before.IsDir() {
		return "", fmt.Errorf("run rootfs %q is not a directory", abs)
	}

	// Persist and execute the resolved target rather than a symlink-bearing
	// pathname. Otherwise a symlink retarget after durable admission could make
	// the runtime execute a different filesystem tree than the one recorded in
	// lifecycle state.
	resolved, err := deps.evalSymlinks(abs)
	if err != nil {
		return "", fmt.Errorf("resolve run rootfs symlinks %q: %w", abs, err)
	}
	resolved = filepath.Clean(resolved)
	after, err := deps.stat(resolved)
	if err != nil {
		return "", fmt.Errorf("stat resolved run rootfs %q: %w", resolved, err)
	}
	if !after.IsDir() {
		return "", fmt.Errorf("resolved run rootfs %q is not a directory", resolved)
	}
	if !os.SameFile(before, after) {
		return "", fmt.Errorf("run rootfs changed while resolving symlinks: %q -> %q", abs, resolved)
	}
	return resolved, nil
}
