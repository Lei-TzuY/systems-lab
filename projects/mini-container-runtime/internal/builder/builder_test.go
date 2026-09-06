package builder

import (
	"os"
	"path/filepath"
	"testing"

	"minicontainer/internal/state"
)

func TestBuildDockerfile(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state error: %v", err)
	}

	contextDir := filepath.Join(tmpDir, "app-src")
	if err := os.MkdirAll(contextDir, 0755); err != nil {
		t.Fatalf("Mkdir context dir error: %v", err)
	}

	// Create sample app file
	if err := os.WriteFile(filepath.Join(contextDir, "index.js"), []byte("console.log('hello minidocker');"), 0644); err != nil {
		t.Fatalf("Write index.js error: %v", err)
	}

	// Create Dockerfile
	dockerfileContent := `
# Sample Dockerfile
FROM alpine:3.19
WORKDIR /app
ENV PORT=8080
COPY index.js .
RUN echo "built" > /app/status.txt
EXPOSE 8080
CMD ["node", "index.js"]
`
	dockerfilePath := filepath.Join(contextDir, "Dockerfile")
	if err := os.WriteFile(dockerfilePath, []byte(dockerfileContent), 0644); err != nil {
		t.Fatalf("Write Dockerfile error: %v", err)
	}

	outDir := filepath.Join(tmpDir, "out-image")
	res, err := BuildDockerfile(BuildOptions{
		ContextDir: contextDir,
		Dockerfile: dockerfilePath,
		Tag:        "test-app:v1",
		OutputDir:  outDir,
		Store:      st,
	})

	if err != nil {
		t.Fatalf("BuildDockerfile error: %v", err)
	}

	if res.Image.Tag != "v1" || res.Image.Repository != "test-app" {
		t.Fatalf("Unexpected image tag/repo: %s/%s", res.Image.Repository, res.Image.Tag)
	}

	if res.Image.WorkDir != "/app" {
		t.Fatalf("Unexpected WorkDir: %s, want /app", res.Image.WorkDir)
	}

	// Check copied file
	copiedFile := filepath.Join(outDir, "app", "index.js")
	if _, err := os.Stat(copiedFile); os.IsNotExist(err) {
		t.Fatalf("Copied file %s does not exist", copiedFile)
	}

	// Check status.txt created by RUN
	statusFile := filepath.Join(outDir, "app", "status.txt")
	if _, err := os.Stat(statusFile); os.IsNotExist(err) {
		t.Fatalf("RUN created file %s does not exist", statusFile)
	}

	// Verify image registered in state store
	fetched, err := st.GetImage("test-app:v1")
	if err != nil || fetched.ID != res.Image.ID {
		t.Fatalf("GetImage from state failed: %v", err)
	}
}
