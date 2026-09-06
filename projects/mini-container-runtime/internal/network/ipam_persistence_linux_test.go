//go:build linux

package network

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
)

func TestIPAMIndependentManagersAllocateUniqueAddresses(t *testing.T) {
	dir := t.TempDir()
	const managerCount = 8
	const allocationCount = 32
	managers := make([]*IPAM, managerCount)
	for i := range managers {
		var err error
		managers[i], err = OpenIPAM(dir)
		if err != nil {
			t.Fatalf("OpenIPAM(%d): %v", i, err)
		}
	}

	results := make(chan string, allocationCount)
	errs := make(chan error, allocationCount)
	var wg sync.WaitGroup
	for i := 0; i < allocationCount; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			ip, err := managers[i%managerCount].AllocateIP("shared", "10.44.0.0/26", fmt.Sprintf("ctr-%02d", i))
			if err != nil {
				errs <- err
				return
			}
			results <- ip
		}(i)
	}
	wg.Wait()
	close(results)
	close(errs)
	for err := range errs {
		t.Errorf("concurrent allocation: %v", err)
	}

	seen := make(map[string]bool)
	for ip := range results {
		if seen[ip] {
			t.Errorf("duplicate allocation %s", ip)
		}
		seen[ip] = true
	}
	if len(seen) != allocationCount {
		t.Fatalf("unique allocations=%d, want %d", len(seen), allocationCount)
	}
}

func TestIPAMRejectsSubnetChangeForExistingNetwork(t *testing.T) {
	ipam, err := OpenIPAM(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if _, err := ipam.AllocateIP("stable", "10.50.0.0/29", "ctr-a"); err != nil {
		t.Fatalf("initial allocation: %v", err)
	}
	if _, err := ipam.AllocateIP("stable", "10.51.0.0/29", "ctr-b"); err == nil || !strings.Contains(err.Error(), "subnet mismatch") {
		t.Fatalf("subnet change error=%v", err)
	}
}

func TestIPAMCorruptPoolFailsWithoutOverwrite(t *testing.T) {
	dir := t.TempDir()
	ipam, err := OpenIPAM(dir)
	if err != nil {
		t.Fatal(err)
	}
	poolPath := filepath.Join(dir, "broken.json")
	original := []byte(`{"subnet":`)
	if err := os.WriteFile(poolPath, original, 0600); err != nil {
		t.Fatal(err)
	}
	if _, err := ipam.AllocateIP("broken", "10.60.0.0/29", "ctr-a"); err == nil || !strings.Contains(err.Error(), "parse IPAM pool") {
		t.Fatalf("corrupt pool error=%v", err)
	}
	after, err := os.ReadFile(poolPath)
	if err != nil {
		t.Fatal(err)
	}
	if string(after) != string(original) {
		t.Fatalf("corrupt pool was overwritten: %q", after)
	}
}

func TestIPAMRejectsSymlinkPoolAndLock(t *testing.T) {
	dir := t.TempDir()
	ipam, err := OpenIPAM(dir)
	if err != nil {
		t.Fatal(err)
	}

	target := filepath.Join(dir, "target")
	if err := os.WriteFile(target, []byte(`{"subnet":"10.70.0.0/29","allocated":{}}`), 0600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(target, filepath.Join(dir, "pool-link.json")); err != nil {
		t.Fatal(err)
	}
	if _, err := ipam.AllocateIP("pool-link", "10.70.0.0/29", "ctr-a"); err == nil || !strings.Contains(err.Error(), "regular file") {
		t.Fatalf("symlink pool error=%v", err)
	}

	if err := os.Symlink(target, filepath.Join(dir, "lock-link.lock")); err != nil {
		t.Fatal(err)
	}
	if _, err := ipam.AllocateIP("lock-link", "10.71.0.0/29", "ctr-b"); err == nil || !strings.Contains(err.Error(), "open IPAM lock") {
		t.Fatalf("symlink lock error=%v", err)
	}
}

func TestIPAMUsesPrivateDirectoryAndFileModes(t *testing.T) {
	parent := t.TempDir()
	dir := filepath.Join(parent, "ipam")
	ipam, err := OpenIPAM(dir)
	if err != nil {
		t.Fatal(err)
	}
	if info, err := os.Stat(dir); err != nil {
		t.Fatal(err)
	} else if got := info.Mode().Perm(); got != 0700 {
		t.Fatalf("directory mode=%#o, want 0700", got)
	}
	if _, err := ipam.AllocateIP("private", "10.72.0.0/29", "ctr-a"); err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"private.json", "private.lock"} {
		info, err := os.Stat(filepath.Join(dir, name))
		if err != nil {
			t.Fatal(err)
		}
		if got := info.Mode().Perm(); got != 0600 {
			t.Fatalf("%s mode=%#o, want 0600", name, got)
		}
	}
	entries, err := os.ReadDir(dir)
	if err != nil {
		t.Fatal(err)
	}
	for _, entry := range entries {
		if strings.Contains(entry.Name(), ".tmp-") {
			t.Fatalf("temporary pool artifact left behind: %s", entry.Name())
		}
	}
}

func TestOpenIPAMRejectsSymlinkDirectory(t *testing.T) {
	parent := t.TempDir()
	realDir := filepath.Join(parent, "real")
	if err := os.Mkdir(realDir, 0700); err != nil {
		t.Fatal(err)
	}
	linkDir := filepath.Join(parent, "link")
	if err := os.Symlink(realDir, linkDir); err != nil {
		t.Fatal(err)
	}
	if _, err := OpenIPAM(linkDir); err == nil || !strings.Contains(err.Error(), "real directory") {
		t.Fatalf("symlink directory error=%v", err)
	}
}

func TestIPAMRejectsEmptyContainerID(t *testing.T) {
	ipam, err := OpenIPAM(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if _, err := ipam.AllocateIP("demo", "10.73.0.0/29", "   "); err == nil || !strings.Contains(err.Error(), "container ID") {
		t.Fatalf("empty allocation owner error=%v", err)
	}
	if err := ipam.ReleaseIP("demo", ""); err == nil || !strings.Contains(err.Error(), "container ID") {
		t.Fatalf("empty release owner error=%v", err)
	}
}
