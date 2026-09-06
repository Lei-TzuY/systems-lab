package cgroups

import (
	"errors"
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadMemorySwapCurrent_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	val, err := ReadMemorySwapCurrent(tmpDir)
	if !errors.Is(err, ErrMemorySwapUnavailable) {
		t.Fatalf("error = %v, want ErrMemorySwapUnavailable", err)
	}
	if val != 0 {
		t.Errorf("val = %d, want 0", val)
	}
}

func TestReadMemorySwapHigh_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	val, isMax, err := ReadMemorySwapHigh(tmpDir)
	if !errors.Is(err, ErrMemorySwapUnavailable) {
		t.Fatalf("error = %v, want ErrMemorySwapUnavailable", err)
	}
	if val != 0 || isMax {
		t.Errorf("val=%d, isMax=%t, want 0, false", val, isMax)
	}
}

func TestWriteMemorySwapHigh_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	err := WriteMemorySwapHigh(tmpDir, 1048576)
	if !errors.Is(err, ErrMemorySwapUnavailable) {
		t.Fatalf("error = %v, want ErrMemorySwapUnavailable", err)
	}
}

func TestReadMemorySwapHigh_Linux(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading is Linux-specific")
	}

	tmpDir := t.TempDir()
	highPath := filepath.Join(tmpDir, "memory.swap.high")
	if err := os.WriteFile(highPath, []byte("max\n"), 0644); err != nil {
		t.Fatal(err)
	}

	val, isMax, err := ReadMemorySwapHigh(tmpDir)
	if err != nil {
		t.Fatal(err)
	}
	if !isMax {
		t.Errorf("expected isMax = true for 'max', got %t", isMax)
	}
	if val != 0 {
		t.Errorf("expected val = 0 for 'max', got %d", val)
	}

	if err := os.WriteFile(highPath, []byte("52428800\n"), 0644); err != nil {
		t.Fatal(err)
	}
	val, isMax, err = ReadMemorySwapHigh(tmpDir)
	if err != nil {
		t.Fatal(err)
	}
	if isMax {
		t.Errorf("expected isMax = false for numeric value, got %t", isMax)
	}
	if val != 52428800 {
		t.Errorf("val = %d, want 52428800", val)
	}
}

func TestWriteMemorySwapHigh_Linux(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file writing is Linux-specific")
	}

	tmpDir := t.TempDir()
	highPath := filepath.Join(tmpDir, "memory.swap.high")
	if err := os.WriteFile(highPath, []byte(""), 0644); err != nil {
		t.Fatal(err)
	}

	if err := WriteMemorySwapHigh(tmpDir, 10485760); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(highPath)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != "10485760\n" {
		t.Errorf("file content = %q, want '10485760\\n'", string(data))
	}

	if err := WriteMemorySwapHigh(tmpDir, -1); err != nil {
		t.Fatal(err)
	}
	data, err = os.ReadFile(highPath)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != "max\n" {
		t.Errorf("file content = %q, want 'max\\n'", string(data))
	}
}
