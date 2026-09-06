package main

import (
	"fmt"
	"os"
	"reflect"

	"minicontainer/internal/state"
)

func imageCommandForRootFS(st *state.Store, rootfs string, cliCommand []string) ([]string, error) {
	if st == nil {
		return nil, fmt.Errorf("state store is nil")
	}
	rootInfo, err := os.Stat(rootfs)
	if err != nil {
		return nil, fmt.Errorf("stat admitted rootfs for image command: %w", err)
	}

	images, err := st.ListImages()
	if err != nil {
		return nil, fmt.Errorf("list images for command: %w", err)
	}

	var selected state.ImageCommand
	selectedKnown := false
	for _, img := range images {
		if img == nil || img.RootFS == "" || img.Name == "" {
			continue
		}
		info, err := os.Stat(img.RootFS)
		if err != nil || !os.SameFile(rootInfo, info) {
			continue
		}

		command, ok, err := st.ImageCommandConfig(img.Name)
		if err != nil {
			return nil, fmt.Errorf("read image %q command: %w", img.Name, err)
		}
		if !ok {
			if len(img.Cmd) == 0 {
				continue
			}
			command.Cmd = append([]string(nil), img.Cmd...)
		}
		if selectedKnown && (!reflect.DeepEqual(selected.Entrypoint, command.Entrypoint) || !reflect.DeepEqual(selected.Cmd, command.Cmd)) {
			return nil, fmt.Errorf("conflicting image command metadata for rootfs %q", rootfs)
		}
		selected = state.ImageCommand{
			Entrypoint: append([]string(nil), command.Entrypoint...),
			Cmd:        append([]string(nil), command.Cmd...),
		}
		selectedKnown = true
	}

	resolved := append([]string(nil), cliCommand...)
	if selectedKnown {
		if len(cliCommand) > 0 {
			if len(selected.Entrypoint) > 0 {
				resolved = append(append([]string(nil), selected.Entrypoint...), cliCommand...)
			}
		} else {
			resolved = append(append([]string(nil), selected.Entrypoint...), selected.Cmd...)
		}
	}
	if len(resolved) == 0 || resolved[0] == "" {
		return nil, fmt.Errorf("container command is empty and image provides no executable Entrypoint/Cmd")
	}
	for _, arg := range resolved {
		if len(arg) > 0 {
			continue
		}
		// Empty non-argv0 arguments are valid and must be preserved.
	}
	return resolved, nil
}
