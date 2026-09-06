//go:build linux

package dns

import (
	"strings"
	"testing"
)

func TestDNSNetworkFilenameLengthBoundary(t *testing.T) {
	valid := strings.Repeat("a", maxDNSNetworkNameBytes)
	if err := validateDNSNetworkFilenameLength(valid); err != nil {
		t.Fatalf("boundary network name rejected: %v", err)
	}
	name, err := dnsTempName(valid)
	if err != nil {
		t.Fatalf("generate boundary temp name: %v", err)
	}
	if got := len(name); got != dnsFilenameMaxBytes {
		t.Fatalf("boundary temp filename length = %d, want %d", got, dnsFilenameMaxBytes)
	}

	overlong := valid + "a"
	if err := validateDNSNetworkFilenameLength(overlong); err == nil {
		t.Fatal("expected overlong network name to be rejected")
	}
	if _, err := dnsTempName(overlong); err == nil {
		t.Fatal("expected temp-name sink to reject overlong network name")
	}
}

func TestDNSNetworkFilenameLengthUsesSmallerFilesystemLimit(t *testing.T) {
	const componentLimit int64 = 143
	limit, err := maxDNSNetworkNameBytesForComponentLimit(componentLimit)
	if err != nil {
		t.Fatalf("derive filesystem network-name limit: %v", err)
	}
	want := int(componentLimit) - dnsRegistryTempFixedBytes
	if limit != want {
		t.Fatalf("filesystem network-name limit = %d, want %d", limit, want)
	}
}

func TestDNSNetworkLockRejectsOverlongNameBeforeFilesystemLookup(t *testing.T) {
	overlong := strings.Repeat("a", maxDNSNetworkNameBytes+1)
	called := false
	err := withDNSNetworkLock("/definitely/not/a/dns/registry", overlong, func(int) error {
		called = true
		return nil
	})
	if err == nil {
		t.Fatal("expected overlong network name to be rejected")
	}
	if called {
		t.Fatal("lock callback ran for overlong network name")
	}
	if !strings.Contains(err.Error(), "DNS registry filename budget") {
		t.Fatalf("unexpected error: %v", err)
	}
}
