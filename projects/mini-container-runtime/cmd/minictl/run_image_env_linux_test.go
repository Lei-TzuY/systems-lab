//go:build linux

package main

import (
	"os/exec"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/container"
	"minicontainer/internal/state"
)

func TestImageEnvironmentForRootFSDrivesRealProcessAndCLIOverride(t *testing.T) {
	stateDir := t.TempDir()
	rootfs := t.TempDir()
	st, err := state.Open(stateDir)
	if err != nil {
		t.Fatalf("state.Open() error = %v", err)
	}
	defer st.Close()
	if err := st.SaveImage(&state.Image{
		Name:     "example:latest",
		RootFS:   rootfs,
		LoadedAt: time.Now(),
		Env:      []string{"FROM_IMAGE=present", "OVERRIDE=image"},
	}); err != nil {
		t.Fatalf("SaveImage() error = %v", err)
	}

	env, err := imageEnvironmentForRootFS(st, rootfs, []string{"OVERRIDE=cli", "CLI_ONLY=yes"})
	if err != nil {
		t.Fatalf("imageEnvironmentForRootFS() error = %v", err)
	}
	cmd := exec.Command("/bin/sh", "-c", "test \"$FROM_IMAGE\" = present && test \"$OVERRIDE\" = cli && test \"$CLI_ONLY\" = yes")
	cmd.Env = env
	if output, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("image environment did not reach process: %v, output=%q, env=%v", err, output, env)
	}
}

func TestPrepareManagedRunStateCommitsImageEnvironment(t *testing.T) {
	stateDir := t.TempDir()
	rootfs := t.TempDir()
	st, err := state.Open(stateDir)
	if err != nil {
		t.Fatalf("state.Open() error = %v", err)
	}
	if err := st.SaveImage(&state.Image{
		Name:     "example:latest",
		RootFS:   rootfs,
		LoadedAt: time.Now(),
		Env:      []string{"FROM_IMAGE=present", "OVERRIDE=image"},
	}); err != nil {
		_ = st.Close()
		t.Fatalf("SaveImage() error = %v", err)
	}
	if err := st.Close(); err != nil {
		t.Fatalf("Close() error = %v", err)
	}

	cfg := container.Config{RootFS: rootfs, Command: []string{"/bin/true"}, Env: []string{"OVERRIDE=cli"}}
	store, rec, err := prepareManagedRunStateWith(&cfg, runAdmissionDeps{
		openStore: func() (*state.Store, error) { return state.Open(stateDir) },
		newID:     func() (string, error) { return "env-admission", nil },
		now:       func() time.Time { return time.Unix(10, 0) },
	})
	if err != nil {
		t.Fatalf("prepareManagedRunStateWith() error = %v", err)
	}
	defer store.Close()

	joined := strings.Join(cfg.Env, "\n")
	if !strings.Contains(joined, "FROM_IMAGE=present") || !strings.Contains(joined, "OVERRIDE=cli") {
		t.Fatalf("runtime env = %v, want image default plus CLI override", cfg.Env)
	}
	if strings.Contains(joined, "OVERRIDE=image") {
		t.Fatalf("runtime env retained overridden image value: %v", cfg.Env)
	}
	if strings.Join(rec.Env, "\n") != joined {
		t.Fatalf("persisted env = %v, runtime env = %v", rec.Env, cfg.Env)
	}
}

func TestImageEnvironmentForRootFSRejectsConflictingTags(t *testing.T) {
	stateDir := t.TempDir()
	rootfs := t.TempDir()
	st, err := state.Open(stateDir)
	if err != nil {
		t.Fatalf("state.Open() error = %v", err)
	}
	defer st.Close()
	for _, img := range []*state.Image{
		{Name: "example:v1", RootFS: rootfs, LoadedAt: time.Now(), Env: []string{"MODE=one"}},
		{Name: "example:v2", RootFS: rootfs, LoadedAt: time.Now(), Env: []string{"MODE=two"}},
	} {
		if err := st.SaveImage(img); err != nil {
			t.Fatalf("SaveImage(%s) error = %v", img.Name, err)
		}
	}

	_, err = imageEnvironmentForRootFS(st, rootfs, nil)
	if err == nil {
		t.Fatal("imageEnvironmentForRootFS() returned nil for conflicting image environment")
	}
	if !strings.Contains(err.Error(), "conflicting image environments") {
		t.Fatalf("error = %q, want conflicting image environments", err)
	}
}
