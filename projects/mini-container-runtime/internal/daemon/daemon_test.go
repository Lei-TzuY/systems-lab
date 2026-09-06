package daemon

import (
	"context"
	"path/filepath"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestDaemonRESTAPI(t *testing.T) {
	tmpDir := t.TempDir()
	sockPath := filepath.Join(tmpDir, "minictl.sock")
	listenAddr := "unix://" + sockPath

	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state error: %v", err)
	}

	// Generic API smoke test uses a stopped record. Live-process lifecycle
	// semantics are covered separately with real pidfd-backed process tests.
	c := &state.Container{
		ID:        "ctr-test-123",
		Status:    state.StatusStopped,
		RootFS:    "/tmp/rootfs",
		Command:   []string{"/bin/sh"},
		CreatedAt: time.Now(),
	}
	if err := st.Save(c); err != nil {
		t.Fatalf("Save dummy container error: %v", err)
	}

	img := &state.Image{
		ID:         "img-test-456",
		Repository: "test-repo",
		Tag:        "v1",
		Name:       "test-repo:v1",
		RootFS:     tmpDir,
		LoadedAt:   time.Now(),
	}
	if err := st.SaveImage(img); err != nil {
		t.Fatalf("Save dummy image error: %v", err)
	}

	server, err := NewServer(Config{
		ListenAddr: listenAddr,
		StoreDir:   tmpDir,
	})
	if err != nil {
		t.Fatalf("NewServer error: %v", err)
	}

	go func() {
		_ = server.Start()
	}()
	defer func() {
		ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
		defer cancel()
		_ = server.Stop(ctx)
	}()

	time.Sleep(100 * time.Millisecond)

	cli := NewClient(listenAddr)

	info, err := cli.SystemInfo()
	if err != nil || info["version"] != "minictl/1.2.0" {
		t.Fatalf("SystemInfo failed: %v, info: %v", err, info)
	}

	ctrs, err := cli.ListContainers()
	if err != nil || len(ctrs) != 1 || ctrs[0].ID != "ctr-test-123" {
		t.Fatalf("ListContainers failed: %v, count: %d", err, len(ctrs))
	}

	imgs, err := cli.ListImages()
	if err != nil || len(imgs) != 1 || imgs[0].Repository != "test-repo" {
		t.Fatalf("ListImages failed: %v", err)
	}

	// Stop is idempotent for an already-stopped container.
	if err := cli.StopContainer("ctr-test-123"); err != nil {
		t.Fatalf("StopContainer error: %v", err)
	}

	updated, err := st.Get("ctr-test-123")
	if err != nil || updated.Status != state.StatusStopped {
		t.Fatalf("Container status after stop = %v, want stopped", updated.Status)
	}
}
