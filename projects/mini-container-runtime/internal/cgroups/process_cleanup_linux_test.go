//go:build linux

package cgroups

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestProcessCleanupPathsV2RequiresExactMembership(t *testing.T) {
	const name = "minicontainer-abc-42-99"
	paths, err := processCleanupPaths(name, "0::/"+name+"\n", true)
	if err != nil {
		t.Fatal(err)
	}
	want := filepath.Join(cgroupV2Root, name)
	if len(paths) != 1 || paths[0] != want {
		t.Fatalf("paths=%v, want [%s]", paths, want)
	}

	paths, err = processCleanupPaths(name, "0::/nested/"+name+"\n", true)
	if err != nil {
		t.Fatal(err)
	}
	if len(paths) != 0 {
		t.Fatalf("nested foreign membership produced cleanup paths: %v", paths)
	}
}

func TestProcessCleanupPathsV1CapturesOnlyManagedControllers(t *testing.T) {
	const name = "minicontainer-abc-42-99"
	membership := strings.Join([]string{
		"2:cpu,cpuacct:/" + name,
		"3:memory:/" + name,
		"4:pids:/other",
		"5:blkio:/" + name,
	}, "\n")
	paths, err := processCleanupPaths(name, membership, false)
	if err != nil {
		t.Fatal(err)
	}
	want := map[string]bool{
		filepath.Join("/sys/fs/cgroup", "cpu", name):    true,
		filepath.Join("/sys/fs/cgroup", "memory", name): true,
	}
	if len(paths) != len(want) {
		t.Fatalf("paths=%v, want %v", paths, want)
	}
	for _, path := range paths {
		if !want[path] {
			t.Fatalf("unexpected cleanup path %q", path)
		}
	}
}

func TestProcessCleanupPathsRejectsMalformedMembership(t *testing.T) {
	if _, err := processCleanupPaths("minicontainer-abc-42-99", "not-a-cgroup-line", true); err == nil {
		t.Fatal("malformed membership accepted")
	}
}

func TestRemoveCgroupPathsReportsFailures(t *testing.T) {
	root := t.TempDir()
	empty := filepath.Join(root, "empty")
	if err := os.Mkdir(empty, 0o755); err != nil {
		t.Fatal(err)
	}
	missing := filepath.Join(root, "missing")
	if err := removeCgroupPaths([]string{empty, missing}, false); err != nil {
		t.Fatalf("remove empty/missing paths: %v", err)
	}
	if _, err := os.Stat(empty); !os.IsNotExist(err) {
		t.Fatalf("empty cgroup path still exists: %v", err)
	}

	nonEmpty := filepath.Join(root, "non-empty")
	if err := os.Mkdir(nonEmpty, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(nonEmpty, "member"), []byte("x"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := removeCgroupPaths([]string{nonEmpty}, false); err == nil {
		t.Fatal("non-empty cgroup cleanup failure was silently discarded")
	}
}
