package dns

import (
	"strings"
	"testing"
)

func TestCanonicalIPAddressCollapsesEquivalentIPv6Spellings(t *testing.T) {
	canonical, err := canonicalIPAddress("2001:db8::1")
	if err != nil {
		t.Fatal(err)
	}
	expanded, err := canonicalIPAddress("2001:0db8:0:0:0:0:0:1")
	if err != nil {
		t.Fatal(err)
	}
	if canonical != "2001:db8::1" || expanded != canonical {
		t.Fatalf("canonical forms = %q and %q, want identical 2001:db8::1", canonical, expanded)
	}
}

func TestCanonicalIPAddressPreservesMappedIPv6Family(t *testing.T) {
	got, err := canonicalIPAddress("::ffff:192.0.2.1")
	if err != nil {
		t.Fatal(err)
	}
	if got != "::ffff:192.0.2.1" {
		t.Fatalf("mapped IPv6 canonical form = %q, want family-preserving ::ffff:192.0.2.1", got)
	}
}

func TestEntriesWithRegistrationCanonicalizesIPAddress(t *testing.T) {
	owner := registrarIdentity{PID: 100, StartTime: 200}
	entries, changed, err := entriesWithRegistration(nil, owner, "c1", "host-a", "2001:0db8:0:0:0:0:0:1", false)
	if err != nil {
		t.Fatal(err)
	}
	if !changed || len(entries) != 1 {
		t.Fatalf("registration changed=%v entries=%+v", changed, entries)
	}
	if entries[0].IP != "2001:db8::1" {
		t.Fatalf("stored IP = %q, want canonical 2001:db8::1", entries[0].IP)
	}
}

func TestValidateEntriesRejectsNonCanonicalIPAddress(t *testing.T) {
	entries := []HostEntry{{ContainerID: "c1", Hostname: "host-a", IP: "2001:0db8:0:0:0:0:0:1"}}
	if err := validateEntries("default", entries); err == nil || !strings.Contains(err.Error(), "non-canonical IP address") {
		t.Fatalf("validateEntries error=%v, want non-canonical IP rejection", err)
	}
}

func TestValidateEntriesAcceptsCanonicalIPAddress(t *testing.T) {
	entries := []HostEntry{{ContainerID: "c1", Hostname: "host-a", IP: "2001:db8::1"}}
	if err := validateEntries("default", entries); err != nil {
		t.Fatalf("canonical registry entry rejected: %v", err)
	}
}
