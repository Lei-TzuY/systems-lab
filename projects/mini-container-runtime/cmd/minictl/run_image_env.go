package main

import (
	"fmt"
	"os"
	"sort"
	"strings"

	"minicontainer/internal/state"
)

// imageEnvironmentForRootFS resolves OCI image environment defaults by the
// filesystem identity admitted for the run. CLI-provided values override image
// defaults by key. Multiple image records sharing one rootfs must agree on
// their environment metadata so runtime behavior is deterministic.
func imageEnvironmentForRootFS(st *state.Store, rootfs string, overrides []string) ([]string, error) {
	if st == nil {
		return nil, fmt.Errorf("state store is nil")
	}
	rootInfo, err := os.Stat(rootfs)
	if err != nil {
		return nil, fmt.Errorf("stat admitted rootfs for image environment: %w", err)
	}

	images, err := st.ListImages()
	if err != nil {
		return nil, fmt.Errorf("list images for environment: %w", err)
	}
	var selected []string
	var selectedSignature string
	selectedKnown := false
	for _, img := range images {
		if img == nil || img.RootFS == "" || img.Name == "" {
			continue
		}
		info, err := os.Stat(img.RootFS)
		if err != nil || !os.SameFile(rootInfo, info) {
			continue
		}

		imageEnv, ok, err := st.ImageEnvironment(img.Name)
		if err != nil {
			return nil, fmt.Errorf("read image %q environment: %w", img.Name, err)
		}
		if !ok {
			if len(img.Env) == 0 {
				continue
			}
			imageEnv = img.Env
		}
		normalized, err := normalizeEnvironment(imageEnv)
		if err != nil {
			return nil, fmt.Errorf("image %q environment: %w", img.Name, err)
		}
		signature := environmentSignature(normalized)
		if selectedKnown && selectedSignature != signature {
			return nil, fmt.Errorf("conflicting image environments for rootfs %q", rootfs)
		}
		selected = normalized
		selectedSignature = signature
		selectedKnown = true
	}

	if !selectedKnown {
		return append([]string(nil), overrides...), nil
	}
	return mergeEnvironment(selected, overrides)
}

func normalizeEnvironment(env []string) ([]string, error) {
	out := make([]string, 0, len(env))
	index := make(map[string]int, len(env))
	for _, entry := range env {
		key, _, ok := strings.Cut(entry, "=")
		if !ok || key == "" || strings.IndexByte(entry, 0) >= 0 {
			return nil, fmt.Errorf("invalid environment entry %q", entry)
		}
		if i, exists := index[key]; exists {
			out[i] = entry
			continue
		}
		index[key] = len(out)
		out = append(out, entry)
	}
	return out, nil
}

func mergeEnvironment(base, overrides []string) ([]string, error) {
	merged, err := normalizeEnvironment(base)
	if err != nil {
		return nil, err
	}
	overrides, err = normalizeEnvironment(overrides)
	if err != nil {
		return nil, err
	}
	index := make(map[string]int, len(merged))
	for i, entry := range merged {
		key, _, _ := strings.Cut(entry, "=")
		index[key] = i
	}
	for _, entry := range overrides {
		key, _, _ := strings.Cut(entry, "=")
		if i, exists := index[key]; exists {
			merged[i] = entry
			continue
		}
		index[key] = len(merged)
		merged = append(merged, entry)
	}
	return merged, nil
}

func environmentSignature(env []string) string {
	canonical := append([]string(nil), env...)
	sort.Strings(canonical)
	return strings.Join(canonical, "\x00")
}
