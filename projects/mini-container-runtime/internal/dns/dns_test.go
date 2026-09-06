package dns

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestContainerDNS(t *testing.T) {
	tmpHome := t.TempDir()
	t.Setenv("HOME", tmpHome)
	t.Setenv("USERPROFILE", tmpHome)

	netName := "demo-net"
	ctrID1 := "ctr111"
	ctrID2 := "ctr222"

	if err := RegisterHost(netName, ctrID1, "web", "172.20.0.2"); err != nil {
		t.Fatalf("RegisterHost ctr1 error: %v", err)
	}
	if err := RegisterHost(netName, ctrID2, "db", "172.20.0.3"); err != nil {
		t.Fatalf("RegisterHost ctr2 error: %v", err)
	}

	hostsContent := GenerateHostsContent(netName)
	if !strings.Contains(hostsContent, "172.20.0.2\tweb") || !strings.Contains(hostsContent, "172.20.0.3\tdb") {
		t.Fatalf("Hosts content missing expected entries:\n%s", hostsContent)
	}

	rootfs := t.TempDir()
	if err := os.MkdirAll(filepath.Join(rootfs, "etc"), 0o755); err != nil {
		t.Fatal(err)
	}
	hostsPath := filepath.Join(rootfs, "etc", "hosts")
	const sentinel = "ORIGINAL ROOTFS HOSTS\n"
	if err := os.WriteFile(hostsPath, []byte(sentinel), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := InjectHostsIntoRootFS(rootfs, netName); err != nil {
		t.Fatalf("legacy hosts validation error: %v", err)
	}
	data, err := os.ReadFile(hostsPath)
	if err != nil || string(data) != sentinel {
		t.Fatalf("legacy injection mutated rootfs hosts: data=%q err=%v", data, err)
	}

	if err := UnregisterHost(netName, ctrID1); err != nil {
		t.Fatalf("UnregisterHost error: %v", err)
	}
	updatedContent := GenerateHostsContent(netName)
	if strings.Contains(updatedContent, "172.20.0.2\tweb") {
		t.Fatalf("Unregistered web host should not be in content:\n%s", updatedContent)
	}
}

func TestDNSValidationAndInjectionDefense(t *testing.T) {
	tmpHome := t.TempDir()
	t.Setenv("HOME", tmpHome)
	t.Setenv("USERPROFILE", tmpHome)

	invalidNetworks := []string{"", ".", "..", "../escape", "../../etc", "foo/bar", "colon:net"}
	for _, net := range invalidNetworks {
		if err := RegisterHost(net, "ctr1", "host1", "172.20.0.2"); err == nil {
			t.Errorf("RegisterHost(%q) expected error, got nil", net)
		}
		if err := UnregisterHost(net, "ctr1"); err == nil {
			t.Errorf("UnregisterHost(%q) expected error, got nil", net)
		}
		if content := GenerateHostsContent(net); content != "" {
			t.Errorf("GenerateHostsContent(%q) expected empty string, got %q", net, content)
		}
	}

	invalidHosts := []string{
		"host\ninjection",
		"host\r\ninjection",
		"host\tinjection",
		"host injection",
		"-bad-leading",
		".bad.dot",
		"",
	}
	for _, h := range invalidHosts {
		if err := RegisterHost("valid-net", "ctr1", h, "172.20.0.2"); err == nil {
			t.Errorf("RegisterHost with invalid hostname %q expected error, got nil", h)
		}
	}

	invalidIPs := []string{
		"not-an-ip",
		"172.20.0.2\n1.2.3.4 evil.com",
		"999.999.999.999",
		"",
	}
	for _, ip := range invalidIPs {
		if err := RegisterHost("valid-net", "ctr1", "valid-host", ip); err == nil {
			t.Errorf("RegisterHost with invalid IP %q expected error, got nil", ip)
		}
	}
}

func TestDNSConcurrency(t *testing.T) {
	tmpHome := t.TempDir()
	t.Setenv("HOME", tmpHome)
	t.Setenv("USERPROFILE", tmpHome)

	netName := "concurrent-net"
	done := make(chan bool)

	for i := 0; i < 10; i++ {
		go func(idx int) {
			ctrID := fmt.Sprintf("ctr-%d", idx)
			hostname := fmt.Sprintf("host-%d", idx)
			ip := fmt.Sprintf("10.0.0.%d", idx+2)
			_ = RegisterHost(netName, ctrID, hostname, ip)
			_ = GenerateHostsContent(netName)
			_ = UnregisterHost(netName, ctrID)
			done <- true
		}(i)
	}

	for i := 0; i < 10; i++ {
		<-done
	}
}

func TestInjectHostsInvalidRootFS(t *testing.T) {
	tmpHome := t.TempDir()
	t.Setenv("HOME", tmpHome)
	t.Setenv("USERPROFILE", tmpHome)

	netName := "test-net"
	_ = RegisterHost(netName, "c1", "host1", "10.0.0.2")

	if err := InjectHostsIntoRootFS("", netName); err == nil {
		t.Errorf("InjectHostsIntoRootFS on empty rootfs expected error, got nil")
	}

	filePath := filepath.Join(tmpHome, "file.txt")
	_ = os.WriteFile(filePath, []byte("regular file"), 0o644)
	if err := InjectHostsIntoRootFS(filePath, netName); err == nil {
		t.Errorf("InjectHostsIntoRootFS on regular file expected error, got nil")
	}

	if err := InjectHostsIntoRootFS(tmpHome, "../bad-net"); err == nil {
		t.Errorf("InjectHostsIntoRootFS with traversal network name expected error, got nil")
	}

	missing := filepath.Join(tmpHome, "missing-rootfs")
	if err := InjectHostsIntoRootFS(missing, netName); err == nil {
		t.Errorf("InjectHostsIntoRootFS on missing rootfs expected error, got nil")
	}
	if _, err := os.Stat(missing); !os.IsNotExist(err) {
		t.Fatalf("legacy injection created missing rootfs: %v", err)
	}
}

func TestInjectHostsNeverMutatesRootFSOrSymlinkTargets(t *testing.T) {
	tmpHome := t.TempDir()
	t.Setenv("HOME", tmpHome)
	t.Setenv("USERPROFILE", tmpHome)

	netName := "sec-net"
	_ = RegisterHost(netName, "c1", "web", "10.0.0.5")

	rootfs := t.TempDir()
	outsideDir := t.TempDir()
	outsideSentinel := filepath.Join(outsideDir, "hosts")
	if err := os.WriteFile(outsideSentinel, []byte("HOST SENTINEL DATA"), 0o644); err != nil {
		t.Fatalf("write sentinel: %v", err)
	}

	etcLink := filepath.Join(rootfs, "etc")
	if err := os.Symlink(outsideDir, etcLink); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	if err := InjectHostsIntoRootFS(rootfs, netName); err != nil {
		t.Fatalf("validation-only injection rejected valid rootfs: %v", err)
	}
	data, err := os.ReadFile(outsideSentinel)
	if err != nil || string(data) != "HOST SENTINEL DATA" {
		t.Fatalf("outside sentinel changed through /etc symlink: data=%q err=%v", data, err)
	}
	fi, err := os.Lstat(etcLink)
	if err != nil || fi.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("legacy injection replaced /etc symlink: info=%v err=%v", fi, err)
	}
}
