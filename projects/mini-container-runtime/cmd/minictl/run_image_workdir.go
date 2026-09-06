package main

import (
	"fmt"
	"os"
	"path/filepath"

	"minicontainer/internal/state"
)

// imageWorkingDirForRootFS resolves the OCI image WorkingDir by the filesystem
// identity admitted for the run. An explicit CLI workdir overrides the image
// default, but conflicting image metadata for the same rootfs still fails
// closed so runtime behavior never depends on image enumeration order.
func imageWorkingDirForRootFS(st *state.Store, rootfs, override string) (string, error) {
	if st == nil {
		return "", fmt.Errorf("state store is nil")
	}
	rootInfo, err := os.Stat(rootfs)
	if err != nil {
		return "", fmt.Errorf("stat admitted rootfs for image WorkingDir: %w", err)
	}

	images, err := st.ListImages()
	if err != nil {
		return "", fmt.Errorf("list images for WorkingDir: %w", err)
	}
	selected := ""
	selectedKnown := false
	for _, img := range images {
		if img == nil || img.RootFS == "" || img.Name == "" {
			continue
		}
		info, err := os.Stat(img.RootFS)
		if err != nil || !os.SameFile(rootInfo, info) {
			continue
		}

		workDir, ok, err := st.ImageWorkingDir(img.Name)
		if err != nil {
			return "", fmt.Errorf("read image %q WorkingDir: %w", img.Name, err)
		}
		if !ok {
			if img.WorkDir == "" {
				continue
			}
			workDir = img.WorkDir
		}
		if workDir != "" {
			if !filepath.IsAbs(workDir) {
				return "", fmt.Errorf("image %q WorkingDir %q is not absolute", img.Name, workDir)
			}
			workDir = filepath.Clean(workDir)
		}
		if selectedKnown && selected != workDir {
			return "", fmt.Errorf("conflicting image WorkingDir metadata for rootfs %q", rootfs)
		}
		selected = workDir
		selectedKnown = true
	}

	if override != "" {
		return override, nil
	}
	if selectedKnown {
		return selected, nil
	}
	return "", nil
}
