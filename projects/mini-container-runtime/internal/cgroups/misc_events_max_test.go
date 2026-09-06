package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadMiscEventsMax_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	events, err := ReadMiscEventsMax(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(events) != 0 {
		t.Errorf("expected empty events for missing file, got %+v", events)
	}
}

func TestReadMiscEventsMax_Success(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	content := "sev max 12\nsev_es max 0\ntdx max 5\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "misc.events"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	events, err := ReadMiscEventsMax(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(events) != 3 {
		t.Fatalf("len(events) = %d, want 3", len(events))
	}
	if events[0].ResourceName != "sev" || events[0].MaxFails != 12 {
		t.Errorf("unexpected event 0: %+v", events[0])
	}
	if events[2].ResourceName != "tdx" || events[2].MaxFails != 5 {
		t.Errorf("unexpected event 2: %+v", events[2])
	}
}
