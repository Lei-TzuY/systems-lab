//go:build linux

package main

import (
	"os/exec"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestImageCommandResolutionExecutesRealProcess(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open() error = %v", err)
	}
	defer st.Close()
	rootfs := t.TempDir()
	if err := st.SaveImage(&state.Image{Name: "example:latest", RootFS: rootfs, LoadedAt: time.Now()}); err != nil {
		t.Fatalf("SaveImage() error = %v", err)
	}
	if err := st.SaveImageCommand("example:latest", state.ImageCommand{
		Entrypoint: []string{"/bin/sh", "-c"},
		Cmd:        []string{"printf '%s' image-command-reached-process"},
	}); err != nil {
		t.Fatalf("SaveImageCommand() error = %v", err)
	}

	argv, err := imageCommandForRootFS(st, rootfs, nil)
	if err != nil {
		t.Fatalf("imageCommandForRootFS() error = %v", err)
	}
	out, err := exec.Command(argv[0], argv[1:]...).Output()
	if err != nil {
		t.Fatalf("exec resolved argv: %v", err)
	}
	if got, want := string(out), "image-command-reached-process"; got != want {
		t.Fatalf("process output = %q, want %q", got, want)
	}
}
