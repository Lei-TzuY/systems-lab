package cgroups

import (
	"os"
	"path/filepath"
	"testing"
)

func TestReadMemoryPSI(t *testing.T) {
	tmpDir := t.TempDir()
	fixture := "some avg10=0.00 avg60=0.00 avg300=0.00 total=0\nfull avg10=0.00 avg60=0.00 avg300=0.00 total=0\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "memory.pressure"), []byte(fixture), 0o644); err != nil {
		t.Fatalf("write memory.pressure fixture: %v", err)
	}
	psi, err := ReadMemoryPSI(tmpDir)
	if err != nil {
		t.Fatalf("ReadMemoryPSI error: %v", err)
	}
	if psi == "" {
		t.Fatalf("ReadMemoryPSI returned empty output")
	}
}
