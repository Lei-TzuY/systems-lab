package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestNormalizeRunAdmissionRootFSRejectsIdentityDriftDuringSymlinkResolution(t *testing.T) {
	beforeDir := t.TempDir()
	afterDir := t.TempDir()
	beforeInfo, err := os.Stat(beforeDir)
	if err != nil {
		t.Fatal(err)
	}
	afterInfo, err := os.Stat(afterDir)
	if err != nil {
		t.Fatal(err)
	}

	const admitted = "/admitted-rootfs"
	const retargeted = "/retargeted-rootfs"
	statCalls := 0
	_, err = normalizeRunAdmissionRootFSWith(admitted, runRootFSAdmissionDeps{
		abs: func(path string) (string, error) {
			return path, nil
		},
		stat: func(path string) (os.FileInfo, error) {
			statCalls++
			switch filepath.Clean(path) {
			case admitted:
				return beforeInfo, nil
			case retargeted:
				return afterInfo, nil
			default:
				t.Fatalf("unexpected stat path %q", path)
				return nil, nil
			}
		},
		evalSymlinks: func(path string) (string, error) {
			if filepath.Clean(path) != admitted {
				t.Fatalf("unexpected eval path %q", path)
			}
			return retargeted, nil
		},
	})
	if err == nil {
		t.Fatal("expected identity drift rejection")
	}
	if !strings.Contains(err.Error(), "changed while resolving symlinks") {
		t.Fatalf("unexpected error: %v", err)
	}
	if statCalls != 2 {
		t.Fatalf("stat calls = %d, want 2", statCalls)
	}
}

func TestNormalizeRunAdmissionRootFSAcceptsStableFilesystemIdentity(t *testing.T) {
	root := t.TempDir()
	info, err := os.Stat(root)
	if err != nil {
		t.Fatal(err)
	}
	clean := filepath.Clean(root)

	got, err := normalizeRunAdmissionRootFSWith(root, runRootFSAdmissionDeps{
		abs: func(string) (string, error) { return clean, nil },
		stat: func(path string) (os.FileInfo, error) {
			if filepath.Clean(path) != clean {
				t.Fatalf("unexpected stat path %q", path)
			}
			return info, nil
		},
		evalSymlinks: func(path string) (string, error) {
			return clean, nil
		},
	})
	if err != nil {
		t.Fatalf("normalize stable rootfs: %v", err)
	}
	if got != clean {
		t.Fatalf("normalized rootfs = %q, want %q", got, clean)
	}
}
