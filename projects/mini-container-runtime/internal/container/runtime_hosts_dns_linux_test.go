//go:build linux

package container

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"minicontainer/internal/dns"
)

func TestCreateRuntimeHostsFileFailsClosedOnCorruptDNSRegistry(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)
	if err := os.MkdirAll(dns.DefaultDNSDir(), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dns.DefaultDNSDir(), bridgeDNSNetworkName+".json"), []byte("{"), 0o600); err != nil {
		t.Fatal(err)
	}

	file, err := createRuntimeHostsFile(true)
	if file != nil {
		_ = file.Close()
		t.Fatal("corrupt DNS registry returned runtime hosts file")
	}
	if err == nil || !strings.Contains(err.Error(), "read bridge DNS registry") {
		t.Fatalf("corrupt DNS registry error=%v", err)
	}
	if !isRuntimeControlError(err) {
		t.Fatalf("DNS registry failure was not runtime-control: %v", err)
	}
}
