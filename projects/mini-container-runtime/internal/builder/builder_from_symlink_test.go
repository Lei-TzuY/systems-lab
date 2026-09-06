package builder

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestBuildFromBaseSymlinkDoesNotDereferenceHostTarget(t *testing.T) {
	base := t.TempDir()
	contextDir := filepath.Join(base, "context")
	if err := os.Mkdir(contextDir, 0o700); err != nil {
		t.Fatal(err)
	}
	baseRoot := filepath.Join(base, "base-root")
	if err := os.Mkdir(baseRoot, 0o700); err != nil {
		t.Fatal(err)
	}

	outside := filepath.Join(base, "outside.txt")
	if err := os.WriteFile(outside, []byte("sentinel"), 0o600); err != nil {
		t.Fatal(err)
	}

	// The base image contains the same absolute target path inside its own root.
	// A real container would resolve /leak to this in-container path, not to the
	// host's /tmp/... path. Creating it also makes the RUN redirection valid.
	insideBaseTarget := filepath.Join(baseRoot, strings.TrimPrefix(filepath.ToSlash(outside), "/"))
	if err := os.MkdirAll(filepath.Dir(insideBaseTarget), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(insideBaseTarget, []byte("base-value"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, filepath.Join(baseRoot, "leak")); err != nil {
		t.Fatal(err)
	}

	outputDir := filepath.Join(base, "output")
	dockerfile := "FROM " + baseRoot + "\nRUN echo contained > /leak\n"
	if err := buildSecurityDockerfile(t, contextDir, outputDir, dockerfile); err != nil {
		t.Fatalf("build from symlinked base tree: %v", err)
	}

	hostData, err := os.ReadFile(outside)
	if err != nil {
		t.Fatal(err)
	}
	if string(hostData) != "sentinel" {
		t.Fatalf("FROM/RUN dereferenced host symlink target: %q", hostData)
	}

	logicalTarget := filepath.Join(outputDir, strings.TrimPrefix(filepath.ToSlash(outside), "/"))
	inside, err := os.ReadFile(logicalTarget)
	if err != nil {
		t.Fatalf("container-relative symlink target missing: %v", err)
	}
	if string(inside) != "contained\n" {
		t.Fatalf("container-relative symlink target=%q", inside)
	}
}
