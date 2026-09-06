//go:build linux

package dns

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func useTempDNSHome(t *testing.T) string {
	t.Helper()
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)
	return home
}

func TestDNSRegistryCorruptionFailsClosedWithoutOverwrite(t *testing.T) {
	useTempDNSHome(t)
	const networkName = "default"
	if err := RegisterHost(networkName, "ctr-one", "web", "10.0.0.2"); err != nil {
		t.Fatal(err)
	}

	path := filepath.Join(DefaultDNSDir(), networkName+".json")
	corrupt := []byte(`{"container_id":`)
	if err := os.WriteFile(path, corrupt, 0o600); err != nil {
		t.Fatal(err)
	}

	if err := RegisterHost(networkName, "ctr-two", "db", "10.0.0.3"); err == nil {
		t.Fatal("RegisterHost overwrote corrupt registry")
	}
	if err := UnregisterHost(networkName, "ctr-one"); err == nil {
		t.Fatal("UnregisterHost overwrote corrupt registry")
	}
	if _, err := GenerateHostsContentChecked(networkName); err == nil {
		t.Fatal("checked hosts generation accepted corrupt registry")
	}
	if got := GenerateHostsContent(networkName); got != "" {
		t.Fatalf("legacy hosts generation on corrupt registry=%q, want empty", got)
	}
	got, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != string(corrupt) {
		t.Fatalf("corrupt registry was replaced: %q", got)
	}
}

func TestDNSRegistrySymlinkFailsClosedWithoutTouchingTarget(t *testing.T) {
	useTempDNSHome(t)
	if _, err := ensureDNSDir(); err != nil {
		t.Fatal(err)
	}
	const networkName = "default"
	outside := filepath.Join(t.TempDir(), "outside.json")
	const sentinel = `[{"container_id":"foreign","hostname":"foreign","ip":"10.9.0.2"}]`
	if err := os.WriteFile(outside, []byte(sentinel), 0o600); err != nil {
		t.Fatal(err)
	}
	registry := filepath.Join(DefaultDNSDir(), networkName+".json")
	if err := os.Symlink(outside, registry); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}

	if err := RegisterHost(networkName, "ctr-one", "web", "10.0.0.2"); err == nil || !strings.Contains(err.Error(), "regular file") {
		t.Fatalf("symlinked registry RegisterHost error=%v", err)
	}
	if err := UnregisterHost(networkName, "foreign"); err == nil || !strings.Contains(err.Error(), "regular file") {
		t.Fatalf("symlinked registry UnregisterHost error=%v", err)
	}
	got, err := os.ReadFile(outside)
	if err != nil || string(got) != sentinel {
		t.Fatalf("outside target changed: data=%q err=%v", got, err)
	}
}

func TestDNSRegistryFilesArePrivateAndDurableLayoutIsRegular(t *testing.T) {
	useTempDNSHome(t)
	const networkName = "default"
	if err := RegisterHost(networkName, "ctr-one", "web", "10.0.0.2"); err != nil {
		t.Fatal(err)
	}

	dirInfo, err := os.Lstat(DefaultDNSDir())
	if err != nil {
		t.Fatal(err)
	}
	if !dirInfo.IsDir() || dirInfo.Mode().Perm() != 0o700 {
		t.Fatalf("DNS dir mode=%v perm=%#o", dirInfo.Mode(), dirInfo.Mode().Perm())
	}
	for _, name := range []string{networkName + ".json", networkName + ".lock"} {
		info, err := os.Lstat(filepath.Join(DefaultDNSDir(), name))
		if err != nil {
			t.Fatal(err)
		}
		if !info.Mode().IsRegular() || info.Mode().Perm() != 0o600 {
			t.Fatalf("%s mode=%v perm=%#o", name, info.Mode(), info.Mode().Perm())
		}
	}
}

func TestDNSUnregisterMissingRegistryIsIdempotentWithoutDataFile(t *testing.T) {
	useTempDNSHome(t)
	const networkName = "default"
	if err := UnregisterHost(networkName, "missing-container"); err != nil {
		t.Fatalf("missing unregister: %v", err)
	}
	if _, err := os.Lstat(filepath.Join(DefaultDNSDir(), networkName+".json")); !os.IsNotExist(err) {
		t.Fatalf("missing unregister created registry data file: %v", err)
	}
}
