package builder

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func writeBuildOwnershipDockerfile(t *testing.T, contextDir, content string) string {
	t.Helper()
	path := filepath.Join(contextDir, "Dockerfile")
	if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
		t.Fatal(err)
	}
	return path
}

func TestBuildSameTagUsesDistinctOutputAndPreservesOldPayload(t *testing.T) {
	base := t.TempDir()
	stateDir := filepath.Join(base, "state")
	st, err := state.Open(stateDir)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	contextDir := filepath.Join(base, "context")
	if err := os.MkdirAll(contextDir, 0o755); err != nil {
		t.Fatal(err)
	}
	dockerfile := writeBuildOwnershipDockerfile(t, contextDir, "FROM scratch\nRUN echo first > /version.txt\n")

	first, err := BuildDockerfile(BuildOptions{ContextDir: contextDir, Dockerfile: dockerfile, Tag: "repeat:latest", Store: st})
	if err != nil {
		t.Fatalf("first build: %v", err)
	}
	firstRoot := first.Image.RootFS
	wantFirstRoot := filepath.Join(stateDir, "images", first.Image.ID, "rootfs")
	if filepath.Clean(firstRoot) != filepath.Clean(wantFirstRoot) {
		t.Fatalf("default rootfs=%q, want managed path %q", firstRoot, wantFirstRoot)
	}
	firstData, err := os.ReadFile(filepath.Join(firstRoot, "version.txt"))
	if err != nil || string(firstData) != "first\n" {
		t.Fatalf("first payload data=%q err=%v", firstData, err)
	}

	writeBuildOwnershipDockerfile(t, contextDir, "FROM scratch\nRUN echo second > /version.txt\n")
	second, err := BuildDockerfile(BuildOptions{ContextDir: contextDir, Dockerfile: dockerfile, Tag: "repeat:latest", Store: st})
	if err != nil {
		t.Fatalf("second build with same tag: %v", err)
	}
	if filepath.Clean(second.Image.RootFS) == filepath.Clean(firstRoot) {
		t.Fatalf("same tag reused old payload path %q", firstRoot)
	}
	if second.Image.ID == first.Image.ID {
		t.Fatalf("two builds unexpectedly reused image ID %q", first.Image.ID)
	}

	firstData, err = os.ReadFile(filepath.Join(firstRoot, "version.txt"))
	if err != nil || string(firstData) != "first\n" {
		t.Fatalf("old payload changed during rebuild: data=%q err=%v", firstData, err)
	}
	secondData, err := os.ReadFile(filepath.Join(second.Image.RootFS, "version.txt"))
	if err != nil || string(secondData) != "second\n" {
		t.Fatalf("second payload data=%q err=%v", secondData, err)
	}

	current, err := st.GetImage("repeat:latest")
	if err != nil {
		t.Fatal(err)
	}
	if current.ID != second.Image.ID || filepath.Clean(current.RootFS) != filepath.Clean(second.Image.RootFS) {
		t.Fatalf("current tag=%+v, want second build %+v", current, second.Image)
	}
	dangling, err := st.GetImage(first.Image.ID)
	if err != nil {
		t.Fatalf("old build lost durable ownership: %v", err)
	}
	if dangling.Name != "" || dangling.Tag != "<none>" || filepath.Clean(dangling.RootFS) != filepath.Clean(firstRoot) {
		t.Fatalf("old build dangling record=%+v", dangling)
	}
}

func TestBuildRejectsDurablyReferencedExplicitOutputBeforeMutation(t *testing.T) {
	base := t.TempDir()
	st, err := state.Open(filepath.Join(base, "state"))
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	output := filepath.Join(base, "existing-output")
	if err := os.MkdirAll(output, 0o755); err != nil {
		t.Fatal(err)
	}
	sentinel := filepath.Join(output, "sentinel.txt")
	if err := os.WriteFile(sentinel, []byte("original\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := st.SaveImage(&state.Image{
		ID:       "existing-build-id",
		Name:     "existing:latest",
		Tag:      "latest",
		RootFS:   output,
		LoadedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}

	contextDir := filepath.Join(base, "context")
	if err := os.MkdirAll(contextDir, 0o755); err != nil {
		t.Fatal(err)
	}
	dockerfile := writeBuildOwnershipDockerfile(t, contextDir, "FROM scratch\nRUN echo changed > /sentinel.txt\n")
	_, err = BuildDockerfile(BuildOptions{
		ContextDir: contextDir,
		Dockerfile: dockerfile,
		Tag:        "new:latest",
		OutputDir:  output,
		Store:      st,
	})
	if err == nil || !strings.Contains(err.Error(), "durable image metadata references an overlapping rootfs") {
		t.Fatalf("referenced output build error=%v", err)
	}
	data, readErr := os.ReadFile(sentinel)
	if readErr != nil || string(data) != "original\n" {
		t.Fatalf("referenced output mutated before rejection: data=%q err=%v", data, readErr)
	}
}

func TestBuildRejectsMissingOutputBelowSymlinkedManagedRoot(t *testing.T) {
	base := t.TempDir()
	st, err := state.Open(filepath.Join(base, "state"))
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	managed := filepath.Join(base, "managed-root")
	if err := os.MkdirAll(managed, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := st.SaveImage(&state.Image{
		ID:       "managed-symlink-id",
		Name:     "managed:latest",
		Tag:      "latest",
		RootFS:   managed,
		LoadedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}
	alias := filepath.Join(base, "alias")
	if err := os.Symlink(managed, alias); err != nil {
		t.Fatal(err)
	}
	output := filepath.Join(alias, "new-child")

	contextDir := filepath.Join(base, "context")
	if err := os.MkdirAll(contextDir, 0o755); err != nil {
		t.Fatal(err)
	}
	dockerfile := writeBuildOwnershipDockerfile(t, contextDir, "FROM scratch\nRUN echo unsafe > /payload.txt\n")
	_, err = BuildDockerfile(BuildOptions{
		ContextDir: contextDir,
		Dockerfile: dockerfile,
		Tag:        "new:latest",
		OutputDir:  output,
		Store:      st,
	})
	if err == nil || !strings.Contains(err.Error(), "overlapping rootfs") {
		t.Fatalf("symlink-overlap build error=%v", err)
	}
	if _, statErr := os.Lstat(filepath.Join(managed, "new-child")); !os.IsNotExist(statErr) {
		t.Fatalf("build created child inside managed rootfs before rejection: %v", statErr)
	}
}

func TestBuildFailureRollsBackNewOwnedOutput(t *testing.T) {
	base := t.TempDir()
	contextDir := filepath.Join(base, "context")
	if err := os.MkdirAll(contextDir, 0o755); err != nil {
		t.Fatal(err)
	}
	dockerfile := writeBuildOwnershipDockerfile(t, contextDir, "FROM scratch\nCOPY missing.txt /missing.txt\n")
	output := filepath.Join(base, "new-output")

	_, err := BuildDockerfile(BuildOptions{
		ContextDir: contextDir,
		Dockerfile: dockerfile,
		Tag:        "failed:latest",
		OutputDir:  output,
	})
	if err == nil {
		t.Fatal("build unexpectedly succeeded")
	}
	if _, statErr := os.Lstat(output); !os.IsNotExist(statErr) {
		t.Fatalf("owned output remained after failed build: %v", statErr)
	}
}

func TestManagedBuildFailureLeavesNoStagingOrMetadata(t *testing.T) {
	base := t.TempDir()
	stateDir := filepath.Join(base, "state")
	st, err := state.Open(stateDir)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	contextDir := filepath.Join(base, "context")
	if err := os.MkdirAll(contextDir, 0o755); err != nil {
		t.Fatal(err)
	}
	dockerfile := writeBuildOwnershipDockerfile(t, contextDir, "FROM scratch\nCOPY missing.txt /missing.txt\n")
	if _, err := BuildDockerfile(BuildOptions{ContextDir: contextDir, Dockerfile: dockerfile, Tag: "failed:managed", Store: st}); err == nil {
		t.Fatal("managed build unexpectedly succeeded")
	}

	entries, err := os.ReadDir(filepath.Join(stateDir, "images"))
	if err != nil {
		t.Fatal(err)
	}
	for _, entry := range entries {
		if entry.IsDir() || strings.HasPrefix(entry.Name(), ".build-") {
			t.Fatalf("managed build artifact remained after failure: %s", entry.Name())
		}
	}
	images, err := st.ListImages()
	if err != nil {
		t.Fatal(err)
	}
	if len(images) != 0 {
		t.Fatalf("failed managed build published metadata: %+v", images)
	}
}

func TestManagedBuildRejectsReplacedStateRoot(t *testing.T) {
	base := t.TempDir()
	stateDir := filepath.Join(base, "state")
	st, err := state.Open(stateDir)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	moved := filepath.Join(base, "state-original")
	if err := os.Rename(stateDir, moved); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(stateDir, "containers"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(stateDir, "images"), 0o755); err != nil {
		t.Fatal(err)
	}

	contextDir := filepath.Join(base, "context")
	if err := os.MkdirAll(contextDir, 0o755); err != nil {
		t.Fatal(err)
	}
	dockerfile := writeBuildOwnershipDockerfile(t, contextDir, "FROM scratch\nRUN echo payload > /payload.txt\n")
	_, err = BuildDockerfile(BuildOptions{ContextDir: contextDir, Dockerfile: dockerfile, Tag: "replaced:state", Store: st})
	if err == nil || !strings.Contains(err.Error(), "changed generation") {
		t.Fatalf("replaced-state managed build error=%v", err)
	}

	replacementEntries, err := os.ReadDir(filepath.Join(stateDir, "images"))
	if err != nil {
		t.Fatal(err)
	}
	if len(replacementEntries) != 0 {
		t.Fatalf("replacement image tree received build artifacts: %+v", replacementEntries)
	}
	originalEntries, err := os.ReadDir(filepath.Join(moved, "images"))
	if err != nil {
		t.Fatal(err)
	}
	if len(originalEntries) != 0 {
		t.Fatalf("pinned original image tree received artifacts before generation rejection: %+v", originalEntries)
	}
}
