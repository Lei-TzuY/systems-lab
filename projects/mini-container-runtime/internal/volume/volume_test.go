package volume

import (
	"os"
	"path/filepath"
	"testing"
)

func TestVolumeManagement(t *testing.T) {
	// Set temp home directory for testing
	tmpHome := t.TempDir()
	t.Setenv("HOME", tmpHome)
	t.Setenv("USERPROFILE", tmpHome)

	volName := "app-db-data"
	vol, err := CreateVolume(volName)
	if err != nil {
		t.Fatalf("CreateVolume error: %v", err)
	}

	if vol.Name != volName {
		t.Fatalf("Volume name = %s, want %s", vol.Name, volName)
	}

	// Write test file to volume mountpath
	testFile := filepath.Join(vol.MountPath, "data.db")
	if err := os.WriteFile(testFile, []byte("sqlite database data"), 0644); err != nil {
		t.Fatalf("Write test file error: %v", err)
	}

	fetched, err := GetVolume(volName)
	if err != nil || fetched.Size == 0 {
		t.Fatalf("GetVolume error: %v, size: %d", err, fetched.Size)
	}

	vols, err := ListVolumes()
	if err != nil || len(vols) != 1 {
		t.Fatalf("ListVolumes count = %d, want 1", len(vols))
	}

	resolvedPath := ResolveVolumePath(volName)
	if resolvedPath != vol.MountPath {
		t.Fatalf("ResolveVolumePath(%s) = %s, want %s", volName, resolvedPath, vol.MountPath)
	}

	if err := RemoveVolume(volName); err != nil {
		t.Fatalf("RemoveVolume error: %v", err)
	}

	if _, err := GetVolume(volName); err == nil {
		t.Fatalf("Volume %s should have been deleted", volName)
	}
}

func TestValidateVolumeName(t *testing.T) {
	validNames := []string{
		"app-db-data",
		"pg_data_123",
		"my.volume.v1",
		"Volume1",
		"a",
	}

	for _, name := range validNames {
		if err := ValidateVolumeName(name); err != nil {
			t.Errorf("ValidateVolumeName(%q) unexpected error: %v", name, err)
		}
	}

	invalidNames := []string{
		"",
		".",
		"..",
		"../escape",
		"../../etc/passwd",
		"foo/bar",
		"foo\\bar",
		"-leading-dash",
		"_leading-underscore",
		".leading-dot",
		"invalid*char",
		"colon:name",
	}

	for _, name := range invalidNames {
		if err := ValidateVolumeName(name); err == nil {
			t.Errorf("ValidateVolumeName(%q) expected error, got nil", name)
		}
	}
}

func TestVolumePathTraversalDefense(t *testing.T) {
	tmpHome := t.TempDir()
	t.Setenv("HOME", tmpHome)
	t.Setenv("USERPROFILE", tmpHome)

	traversalNames := []string{
		"../escape",
		"../../etc",
		"foo/bar",
		"foo\\bar",
		"",
		".",
		"..",
	}

	for _, name := range traversalNames {
		if _, err := CreateVolume(name); err == nil {
			t.Errorf("CreateVolume(%q) expected error, got nil", name)
		}
		if _, err := GetVolume(name); err == nil {
			t.Errorf("GetVolume(%q) expected error, got nil", name)
		}
		if err := RemoveVolume(name); err == nil {
			t.Errorf("RemoveVolume(%q) expected error, got nil", name)
		}
	}

	// ResolveVolumePath with a host path containing separators should return the path as-is
	hostPath := "/var/lib/data"
	if got := ResolveVolumePath(hostPath); got != hostPath {
		t.Errorf("ResolveVolumePath(%q) = %q, want %q", hostPath, got, hostPath)
	}
}
