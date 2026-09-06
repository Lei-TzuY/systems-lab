package builder

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func writeDockerfileForSecurityTest(t *testing.T, contextDir, content string) string {
	t.Helper()
	path := filepath.Join(contextDir, "Dockerfile")
	if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
		t.Fatal(err)
	}
	return path
}

func buildSecurityDockerfile(t *testing.T, contextDir, outputDir, content string) error {
	t.Helper()
	_, err := BuildDockerfile(BuildOptions{ContextDir: contextDir, Dockerfile: writeDockerfileForSecurityTest(t, contextDir, content), Tag: "security:test", OutputDir: outputDir})
	return err
}

func TestBuildRejectsCopySourceTraversal(t *testing.T) {
	base := t.TempDir(); contextDir := filepath.Join(base, "context")
	if err := os.Mkdir(contextDir, 0o700); err != nil { t.Fatal(err) }
	if err := os.WriteFile(filepath.Join(base, "secret.txt"), []byte("host-secret"), 0o600); err != nil { t.Fatal(err) }
	outputDir := filepath.Join(base, "output")
	err := buildSecurityDockerfile(t, contextDir, outputDir, "FROM scratch\nCOPY ../secret.txt /leak.txt\n")
	if err == nil || !strings.Contains(err.Error(), "escapes build context") { t.Fatalf("COPY traversal error=%v", err) }
	if _, statErr := os.Stat(filepath.Join(outputDir, "leak.txt")); !os.IsNotExist(statErr) { t.Fatalf("traversal copied host secret: %v", statErr) }
}

func TestBuildRejectsCopySourceSymlinkOutsideContext(t *testing.T) {
	base := t.TempDir(); contextDir := filepath.Join(base, "context")
	if err := os.Mkdir(contextDir, 0o700); err != nil { t.Fatal(err) }
	outside := filepath.Join(base, "outside.txt")
	if err := os.WriteFile(outside, []byte("outside"), 0o600); err != nil { t.Fatal(err) }
	if err := os.Symlink(outside, filepath.Join(contextDir, "link.txt")); err != nil { t.Fatal(err) }
	err := buildSecurityDockerfile(t, contextDir, filepath.Join(base, "output"), "FROM scratch\nCOPY link.txt /leak.txt\n")
	if err == nil || !strings.Contains(err.Error(), "symlink") { t.Fatalf("COPY symlink error=%v", err) }
}

func TestBuildRejectsSymlinkInsideCopiedContextTree(t *testing.T) {
	base := t.TempDir(); contextDir := filepath.Join(base, "context"); srcDir := filepath.Join(contextDir, "src")
	if err := os.MkdirAll(srcDir, 0o700); err != nil { t.Fatal(err) }
	outside := filepath.Join(base, "outside.txt")
	if err := os.WriteFile(outside, []byte("outside"), 0o600); err != nil { t.Fatal(err) }
	if err := os.Symlink(outside, filepath.Join(srcDir, "leak")); err != nil { t.Fatal(err) }
	err := buildSecurityDockerfile(t, contextDir, filepath.Join(base, "output"), "FROM scratch\nCOPY src /app\n")
	if err == nil || !strings.Contains(err.Error(), "contains symlink") { t.Fatalf("COPY tree symlink error=%v", err) }
}

func TestBuildParentTraversalStaysInsideOutputRoot(t *testing.T) {
	base := t.TempDir(); contextDir := filepath.Join(base, "context")
	if err := os.Mkdir(contextDir, 0o700); err != nil { t.Fatal(err) }
	outputDir := filepath.Join(base, "output"); outside := filepath.Join(base, "pwned.txt")
	if err := os.WriteFile(outside, []byte("sentinel"), 0o600); err != nil { t.Fatal(err) }
	if err := buildSecurityDockerfile(t, contextDir, outputDir, "FROM scratch\nWORKDIR ../../escaped\nRUN echo safe > ../../pwned.txt\n"); err != nil { t.Fatalf("contained traversal build failed: %v", err) }
	data, err := os.ReadFile(outside); if err != nil { t.Fatal(err) }; if string(data) != "sentinel" { t.Fatalf("host sibling overwritten: %q", data) }
	inside, err := os.ReadFile(filepath.Join(outputDir, "pwned.txt")); if err != nil { t.Fatalf("contained output missing: %v", err) }; if string(inside) != "safe\n" { t.Fatalf("inside=%q", inside) }
}

func TestBuildTreatsAbsoluteRootFSSymlinkAsContainerRelative(t *testing.T) {
	base := t.TempDir(); contextDir := filepath.Join(base, "context")
	if err := os.Mkdir(contextDir, 0o700); err != nil { t.Fatal(err) }
	outputDir := filepath.Join(base, "output"); if err := os.Mkdir(outputDir, 0o700); err != nil { t.Fatal(err) }
	outsideDir := filepath.Join(base, "outside"); if err := os.Mkdir(outsideDir, 0o700); err != nil { t.Fatal(err) }
	outsideFile := filepath.Join(outsideDir, "written.txt"); if err := os.WriteFile(outsideFile, []byte("sentinel"), 0o600); err != nil { t.Fatal(err) }
	if err := os.Symlink(outsideDir, filepath.Join(outputDir, "escape")); err != nil { t.Fatal(err) }
	if err := buildSecurityDockerfile(t, contextDir, outputDir, "FROM scratch\nRUN echo contained > /escape/written.txt\n"); err != nil { t.Fatalf("build through rootfs symlink: %v", err) }
	outsideData, err := os.ReadFile(outsideFile); if err != nil { t.Fatal(err) }; if string(outsideData) != "sentinel" { t.Fatalf("host symlink target overwritten: %q", outsideData) }
	logicalTarget := filepath.Join(outputDir, strings.TrimPrefix(filepath.ToSlash(outsideDir), "/"), "written.txt")
	insideData, err := os.ReadFile(logicalTarget); if err != nil { t.Fatalf("container-relative target missing: %v", err) }; if string(insideData) != "contained\n" { t.Fatalf("inside=%q", insideData) }
}
