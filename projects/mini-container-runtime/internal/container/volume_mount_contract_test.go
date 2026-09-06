//go:build linux

package container

import "testing"

func TestMountVolumeRejectsRelativeHostPathBeforeMount(t *testing.T) {
	err := mountVolume(Volume{
		HostPath:      "relative/source",
		ContainerPath: "/data",
	}, t.TempDir(), false)
	if err == nil {
		t.Fatal("relative host path unexpectedly accepted")
	}
}

func TestMountVolumeRejectsTraversalBeforeMount(t *testing.T) {
	err := mountVolume(Volume{
		HostPath:      t.TempDir(),
		ContainerPath: "/safe/../../outside",
	}, t.TempDir(), false)
	if err == nil {
		t.Fatal("container traversal target unexpectedly accepted")
	}
}
