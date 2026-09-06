package main

import (
	"strings"
	"testing"

	"minicontainer/internal/state"
)

func TestImageWorkingDirForRootFSUsesImageDefaultAndCLIOverride(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open() error = %v", err)
	}
	defer st.Close()
	rootfs := t.TempDir()
	if err := st.SaveImage(&state.Image{Name: "example:latest", RootFS: rootfs}); err != nil {
		t.Fatalf("SaveImage() error = %v", err)
	}
	if err := st.SaveImageWorkingDir("example:latest", "/srv/app"); err != nil {
		t.Fatalf("SaveImageWorkingDir() error = %v", err)
	}

	got, err := imageWorkingDirForRootFS(st, rootfs, "")
	if err != nil || got != "/srv/app" {
		t.Fatalf("image default = %q, %v; want /srv/app, nil", got, err)
	}
	got, err = imageWorkingDirForRootFS(st, rootfs, "/cli")
	if err != nil || got != "/cli" {
		t.Fatalf("CLI override = %q, %v; want /cli, nil", got, err)
	}
}

func TestImageWorkingDirForRootFSRejectsConflictingMetadata(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open() error = %v", err)
	}
	defer st.Close()
	rootfs := t.TempDir()
	for _, tc := range []struct {
		name string
		dir  string
	}{{"one:latest", "/one"}, {"two:latest", "/two"}} {
		if err := st.SaveImage(&state.Image{Name: tc.name, RootFS: rootfs}); err != nil {
			t.Fatalf("SaveImage(%s) error = %v", tc.name, err)
		}
		if err := st.SaveImageWorkingDir(tc.name, tc.dir); err != nil {
			t.Fatalf("SaveImageWorkingDir(%s) error = %v", tc.name, err)
		}
	}

	_, err = imageWorkingDirForRootFS(st, rootfs, "/cli")
	if err == nil || !strings.Contains(err.Error(), "conflicting image WorkingDir") {
		t.Fatalf("conflicting metadata error = %v", err)
	}
}
