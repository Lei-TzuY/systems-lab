package main

import (
	"fmt"
	"os"

	"minicontainer/internal/state"
)

// imageStopSignalForRootFS resolves image-level StopSignal metadata by the
// filesystem identity admitted for the run. Multiple tags may share a rootfs;
// conflicting signals fail closed rather than making shutdown behavior depend
// on image enumeration order.
func imageStopSignalForRootFS(st *state.Store, rootfs string) (string, error) {
	if st == nil {
		return "", fmt.Errorf("state store is nil")
	}
	rootInfo, err := os.Stat(rootfs)
	if err != nil {
		return "", fmt.Errorf("stat admitted rootfs for image stop signal: %w", err)
	}

	images, err := st.ListImages()
	if err != nil {
		return "", fmt.Errorf("list images for stop signal: %w", err)
	}
	var selected string
	for _, img := range images {
		if img == nil || img.RootFS == "" || img.Name == "" {
			continue
		}
		info, err := os.Stat(img.RootFS)
		if err != nil || !os.SameFile(rootInfo, info) {
			continue
		}
		signal, ok, err := st.ImageStopSignal(img.Name)
		if err != nil {
			return "", fmt.Errorf("read image %q stop signal: %w", img.Name, err)
		}
		if !ok {
			continue
		}
		if selected != "" && selected != signal {
			return "", fmt.Errorf("conflicting image stop signals for rootfs %q: %s and %s", rootfs, selected, signal)
		}
		selected = signal
	}
	return selected, nil
}
