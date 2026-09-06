package main

import (
	"reflect"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestImageCommandForRootFSUsesEntrypointAndCmd(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open() error = %v", err)
	}
	defer st.Close()
	rootfs := t.TempDir()
	if err := st.SaveImage(&state.Image{Name: "example:latest", RootFS: rootfs, LoadedAt: time.Now()}); err != nil {
		t.Fatalf("SaveImage() error = %v", err)
	}
	if err := st.SaveImageCommand("example:latest", state.ImageCommand{Entrypoint: []string{"/bin/app"}, Cmd: []string{"serve"}}); err != nil {
		t.Fatalf("SaveImageCommand() error = %v", err)
	}

	got, err := imageCommandForRootFS(st, rootfs, nil)
	if err != nil {
		t.Fatalf("imageCommandForRootFS() error = %v", err)
	}
	if want := []string{"/bin/app", "serve"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("command = %#v, want %#v", got, want)
	}
}

func TestImageCommandForRootFSCLIReplacesCmdButKeepsEntrypoint(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open() error = %v", err)
	}
	defer st.Close()
	rootfs := t.TempDir()
	if err := st.SaveImage(&state.Image{Name: "example:latest", RootFS: rootfs, LoadedAt: time.Now()}); err != nil {
		t.Fatalf("SaveImage() error = %v", err)
	}
	if err := st.SaveImageCommand("example:latest", state.ImageCommand{Entrypoint: []string{"/bin/app"}, Cmd: []string{"serve"}}); err != nil {
		t.Fatalf("SaveImageCommand() error = %v", err)
	}

	got, err := imageCommandForRootFS(st, rootfs, []string{"debug", "--once"})
	if err != nil {
		t.Fatalf("imageCommandForRootFS() error = %v", err)
	}
	if want := []string{"/bin/app", "debug", "--once"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("command = %#v, want %#v", got, want)
	}
}

func TestImageCommandForRootFSRejectsConflictingMetadata(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open() error = %v", err)
	}
	defer st.Close()
	rootfs := t.TempDir()
	for _, tc := range []struct {
		name string
		cmd  []string
	}{{"one:latest", []string{"one"}}, {"two:latest", []string{"two"}}} {
		if err := st.SaveImage(&state.Image{Name: tc.name, RootFS: rootfs, LoadedAt: time.Now()}); err != nil {
			t.Fatalf("SaveImage(%q) error = %v", tc.name, err)
		}
		if err := st.SaveImageCommand(tc.name, state.ImageCommand{Cmd: tc.cmd}); err != nil {
			t.Fatalf("SaveImageCommand(%q) error = %v", tc.name, err)
		}
	}

	_, err = imageCommandForRootFS(st, rootfs, []string{"explicit"})
	if err == nil || !strings.Contains(err.Error(), "conflicting image command metadata") {
		t.Fatalf("error = %v, want conflict", err)
	}
}

func TestRootFSOnlyRunConfigResolvesImageDefaults(t *testing.T) {
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
		Entrypoint: []string{"/bin/app"},
		Cmd:        []string{"serve", "--foreground"},
	}); err != nil {
		t.Fatalf("SaveImageCommand() error = %v", err)
	}

	cfg, err := parseRunConfig([]string{rootfs})
	if err != nil {
		t.Fatalf("parseRunConfig rootfs-only error = %v", err)
	}
	got, err := imageCommandForRootFS(st, cfg.RootFS, cfg.Command)
	if err != nil {
		t.Fatalf("imageCommandForRootFS() error = %v", err)
	}
	if want := []string{"/bin/app", "serve", "--foreground"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("resolved command = %#v, want %#v", got, want)
	}
}
