package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadHugeTLBEventsMaxCount_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadHugeTLBEventsMaxCount(tmpDir, "2MB")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if count != 0 {
		t.Errorf("expected 0 for missing file, got %d", count)
	}
}

func TestReadHugeTLBCurrentBytes_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	bytes, err := ReadHugeTLBCurrentBytes(tmpDir, "2MB")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if bytes != 0 {
		t.Errorf("expected 0 for missing file, got %d", bytes)
	}
}

func TestReadHugeTLBEvents_Linux(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	if err := os.WriteFile(filepath.Join(tmpDir, "hugetlb.2MB.events"), []byte("max 15\n"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(tmpDir, "hugetlb.2MB.current"), []byte("20971520\n"), 0644); err != nil {
		t.Fatal(err)
	}

	count, err := ReadHugeTLBEventsMaxCount(tmpDir, "2MB")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if count != 15 {
		t.Errorf("expected 15, got %d", count)
	}

	curBytes, err := ReadHugeTLBCurrentBytes(tmpDir, "2MB")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if curBytes != 20971520 {
		t.Errorf("expected 20971520, got %d", curBytes)
	}
}
