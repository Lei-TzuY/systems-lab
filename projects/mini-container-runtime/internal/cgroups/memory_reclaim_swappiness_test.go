package cgroups

import (
	"errors"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestReclaimMemoryWithOptions_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	opts := MemoryReclaimOptions{
		BytesToReclaim: 1048576,
		Swappiness:     -1,
		NumaNode:       -1,
	}

	err := ReclaimMemoryWithOptions(tmpDir, opts)
	if !errors.Is(err, ErrMemoryReclaimUnavailable) {
		t.Fatalf("error = %v, want ErrMemoryReclaimUnavailable", err)
	}
}

func TestReclaimMemoryWithOptions_EmptyPath(t *testing.T) {
	opts := MemoryReclaimOptions{
		BytesToReclaim: 1048576,
		Swappiness:     -1,
		NumaNode:       -1,
	}

	if err := ReclaimMemoryWithOptions("", opts); err == nil {
		t.Fatal("expected error for empty cgroup path")
	}
}

func TestReclaimMemoryWithOptions_InvalidOptions(t *testing.T) {
	tmpDir := t.TempDir()

	tests := []struct {
		name string
		opts MemoryReclaimOptions
	}{
		{
			name: "invalid negative swappiness",
			opts: MemoryReclaimOptions{BytesToReclaim: 1000, Swappiness: -5, NumaNode: -1},
		},
		{
			name: "invalid excessive swappiness",
			opts: MemoryReclaimOptions{BytesToReclaim: 1000, Swappiness: 201, NumaNode: -1},
		},
		{
			name: "invalid negative numa node",
			opts: MemoryReclaimOptions{BytesToReclaim: 1000, Swappiness: -1, NumaNode: -2},
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			if runtime.GOOS != "linux" {
				t.Skip("option validation only reaches execution check on linux")
			}
			if err := ReclaimMemoryWithOptions(tmpDir, tc.opts); err == nil {
				t.Fatal("expected validation error")
			}
		})
	}
}

func TestReclaimMemoryWithOptions_Linux(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file writing only works on Linux")
	}

	tmpDir := t.TempDir()
	reclaimPath := filepath.Join(tmpDir, "memory.reclaim")
	if err := os.WriteFile(reclaimPath, []byte(""), 0644); err != nil {
		t.Fatal(err)
	}

	opts := MemoryReclaimOptions{
		BytesToReclaim: 2097152,
		Swappiness:     0,
		NumaNode:       1,
	}

	if err := ReclaimMemoryWithOptions(tmpDir, opts); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	data, err := os.ReadFile(reclaimPath)
	if err != nil {
		t.Fatal(err)
	}

	content := strings.TrimSpace(string(data))
	expected := "2097152 swappiness=0 node=1"
	if content != expected {
		t.Errorf("got %q, want %q", content, expected)
	}
}

func TestReclaimMemoryWithOptions_Defaults_Linux(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file writing only works on Linux")
	}

	tmpDir := t.TempDir()
	reclaimPath := filepath.Join(tmpDir, "memory.reclaim")
	if err := os.WriteFile(reclaimPath, []byte(""), 0644); err != nil {
		t.Fatal(err)
	}

	opts := MemoryReclaimOptions{
		BytesToReclaim: 0,
		Swappiness:     -1,
		NumaNode:       -1,
	}

	if err := ReclaimMemoryWithOptions(tmpDir, opts); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	data, err := os.ReadFile(reclaimPath)
	if err != nil {
		t.Fatal(err)
	}

	content := strings.TrimSpace(string(data))
	if content != "1048576" {
		t.Errorf("got %q, want '1048576'", content)
	}
}
