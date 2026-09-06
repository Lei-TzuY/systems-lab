package imagestore

import (
	"strings"
	"testing"
)

func TestDetectPortConflicts(t *testing.T) {
	img1 := `{"config":{"ExposedPorts":{"80/tcp":{},"8080/tcp":{},"90/tcp":{}}}}`
	img2 := `{"config":{"ExposedPorts":{"8080/tcp":{},"443/tcp":{},"90/tcp":{}}}}`

	configs := map[string][]byte{
		"web-frontend": []byte(img1),
		"api-gateway":  []byte(img2),
	}

	conflicts, err := DetectPortConflicts(configs)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(conflicts) != 2 {
		t.Fatalf("expected 2 conflicts, got %d", len(conflicts))
	}

	// Verify numerical sort order: 90/tcp should come BEFORE 8080/tcp
	if conflicts[0].Port != "90/tcp" {
		t.Errorf("conflicts[0].Port = %q, want 90/tcp", conflicts[0].Port)
	}
	if conflicts[1].Port != "8080/tcp" {
		t.Errorf("conflicts[1].Port = %q, want 8080/tcp", conflicts[1].Port)
	}
}

func TestDetectPortConflicts_NoSelfConflictOnDuplicateDeclarations(t *testing.T) {
	img := `{"config":{"ExposedPorts":{"80/tcp":{}}}}`
	configs := map[string][]byte{
		"standalone-app": []byte(img),
	}

	conflicts, err := DetectPortConflicts(configs)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(conflicts) != 0 {
		t.Errorf("expected 0 conflicts for single image, got %d", len(conflicts))
	}
}

func TestFormatPortConflicts(t *testing.T) {
	conflicts := []PortConflict{
		{Port: "80/tcp", Images: []string{"app1", "app2"}},
	}
	got := FormatPortConflicts(conflicts)
	if !strings.Contains(got, "Port Conflicts: 1 overlapping ports") {
		t.Errorf("expected header in %q", got)
	}
	if !strings.Contains(got, "80/tcp -> app1, app2") {
		t.Errorf("expected conflict details in %q", got)
	}
}

func TestFormatPortConflicts_Empty(t *testing.T) {
	got := FormatPortConflicts(nil)
	if got != "Port Conflicts: (none detected)" {
		t.Errorf("got %q, want 'Port Conflicts: (none detected)'", got)
	}
}
