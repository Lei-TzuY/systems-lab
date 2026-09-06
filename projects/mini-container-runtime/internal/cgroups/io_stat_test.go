package cgroups

import (
	"os"
	"path/filepath"
	"testing"
)

func TestReadIOStat(t *testing.T) {
	tmpDir := t.TempDir()
	if err := os.WriteFile(filepath.Join(tmpDir, "io.stat"), []byte("8:0 rbytes=10 wbytes=20 rios=1 wios=2\n"), 0o644); err != nil {
		t.Fatalf("write io.stat fixture: %v", err)
	}
	metrics, err := ReadIOStat(tmpDir)
	if err != nil {
		t.Fatalf("ReadIOStat error: %v", err)
	}
	if metrics["rbytes"] != 10 || metrics["wbytes"] != 20 {
		t.Fatalf("unexpected IO metrics: %#v", metrics)
	}
}
