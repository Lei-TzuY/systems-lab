package builder

import (
	"fmt"
	"os"
	"path/filepath"

	"minicontainer/internal/state"
)

type buildOutputPlan struct {
	path  string
	owned bool
}

func prepareBuildOutput(opts BuildOptions, imageID string) (buildOutputPlan, error) {
	output := opts.OutputDir
	if output == "" {
		home, err := os.UserHomeDir()
		if err != nil || home == "" {
			home = "/tmp"
		}
		// Tags are mutable names. A build payload must therefore never reuse a
		// tag-derived filesystem path whose previous generation may still be
		// referenced by dangling metadata after the tag moves.
		output = filepath.Join(home, ".minicontainer", "builds", imageID)
	}

	abs, err := filepath.Abs(output)
	if err != nil {
		return buildOutputPlan{}, fmt.Errorf("absolute build output path: %w", err)
	}
	output = filepath.Clean(abs)

	if opts.Store != nil {
		referenced, err := buildOutputOverlapsImage(opts.Store, output)
		if err != nil {
			return buildOutputPlan{}, fmt.Errorf("verify build output ownership: %w", err)
		}
		if referenced {
			return buildOutputPlan{}, fmt.Errorf("refusing to mutate build output %q because durable image metadata references an overlapping rootfs", output)
		}
	}

	owned := false
	if _, err := os.Lstat(output); err != nil {
		if !os.IsNotExist(err) {
			return buildOutputPlan{}, fmt.Errorf("inspect build output %q: %w", output, err)
		}
		if err := os.MkdirAll(filepath.Dir(output), 0o755); err != nil {
			return buildOutputPlan{}, fmt.Errorf("create build output parent: %w", err)
		}
		// Claim the final leaf exclusively. If another build creates the same
		// pathname between preflight and creation, this actor never acquires
		// cleanup ownership over that other build's directory.
		if err := os.Mkdir(output, 0o755); err != nil {
			return buildOutputPlan{}, fmt.Errorf("claim build output dir %q: %w", output, err)
		}
		owned = true
	}
	if _, err := canonicalBuildRoot(output); err != nil {
		if owned {
			_ = os.RemoveAll(output)
		}
		return buildOutputPlan{}, fmt.Errorf("validate build output dir: %w", err)
	}
	return buildOutputPlan{path: output, owned: owned}, nil
}

func pathOverlaps(a, b string) bool {
	a = filepath.Clean(a)
	b = filepath.Clean(b)
	if a == b {
		return true
	}
	if rel, err := filepath.Rel(a, b); err == nil && rel != "." && !startsWithParent(rel) {
		return true
	}
	if rel, err := filepath.Rel(b, a); err == nil && rel != "." && !startsWithParent(rel) {
		return true
	}
	return false
}

func startsWithParent(rel string) bool {
	return rel == ".." || len(rel) > 3 && rel[:3] == ".."+string(filepath.Separator)
}

// resolveOwnershipPath resolves every existing ancestor, including symlinks,
// while preserving any not-yet-created suffix. This lets ownership checks see
// that /alias/new would land below a managed rootfs even when only /alias
// exists and is a symlink.
func resolveOwnershipPath(value string) (string, error) {
	abs, err := filepath.Abs(value)
	if err != nil {
		return "", err
	}
	current := filepath.Clean(abs)
	missing := make([]string, 0)
	for {
		resolved, err := filepath.EvalSymlinks(current)
		if err == nil {
			for i := len(missing) - 1; i >= 0; i-- {
				resolved = filepath.Join(resolved, missing[i])
			}
			return filepath.Clean(resolved), nil
		}
		if !os.IsNotExist(err) {
			return "", err
		}
		parent := filepath.Dir(current)
		if parent == current {
			return "", err
		}
		missing = append(missing, filepath.Base(current))
		current = parent
	}
}

func buildOutputOverlapsImage(st *state.Store, output string) (bool, error) {
	images, err := st.ListImages()
	if err != nil {
		return false, err
	}
	outputAbs, err := filepath.Abs(output)
	if err != nil {
		return false, err
	}
	outputAbs = filepath.Clean(outputAbs)
	resolvedOutput, err := resolveOwnershipPath(output)
	if err != nil {
		return false, fmt.Errorf("resolve output filesystem path: %w", err)
	}

	for _, img := range images {
		if img == nil || img.RootFS == "" {
			continue
		}
		rootAbs, err := filepath.Abs(img.RootFS)
		if err != nil {
			return false, fmt.Errorf("absolute image rootfs %q: %w", img.RootFS, err)
		}
		rootAbs = filepath.Clean(rootAbs)
		if pathOverlaps(outputAbs, rootAbs) {
			return true, nil
		}

		resolvedRoot, err := resolveOwnershipPath(img.RootFS)
		if err != nil {
			return false, fmt.Errorf("resolve image rootfs %q: %w", img.RootFS, err)
		}
		if pathOverlaps(resolvedOutput, resolvedRoot) {
			return true, nil
		}
	}
	return false, nil
}

func buildPayloadHasCommittedReference(st *state.Store, imageID, rootFS string) (bool, error) {
	images, err := st.ListImages()
	if err != nil {
		return false, err
	}
	wantRootFS := filepath.Clean(rootFS)
	found := false
	for _, img := range images {
		if img == nil || filepath.Clean(img.RootFS) != wantRootFS {
			continue
		}
		if img.ID != imageID {
			return false, fmt.Errorf("build rootfs %q is referenced by unexpected image ID %q", rootFS, img.ID)
		}
		found = true
	}
	return found, nil
}
