package system

import (
	"testing"

	"minicontainer/internal/state"
)

func TestGenerateEngineReport(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	report, err := GenerateEngineReport(st)
	if err != nil {
		t.Fatalf("GenerateEngineReport error: %v", err)
	}

	if report.EngineVersion == "" {
		t.Fatalf("EngineVersion is empty")
	}
}
