package dns

import (
	"strings"
	"testing"
)

func TestValidateEntriesRejectsCaseInsensitiveHostnameCollision(t *testing.T) {
	entries := []HostEntry{
		{ContainerID: "c1", Hostname: "Api.Service", IP: "10.0.0.2"},
		{ContainerID: "c2", Hostname: "api.service", IP: "10.0.0.3"},
	}
	if err := validateEntries("default", entries); err == nil || !strings.Contains(err.Error(), "case-insensitive") {
		t.Fatalf("validateEntries error=%v, want case-insensitive hostname collision rejection", err)
	}
}

func TestEntriesWithRegistrationTreatsHostnameCaseInsensitively(t *testing.T) {
	owner := registrarIdentity{PID: 100, StartTime: 200}
	entries := []HostEntry{{
		ContainerID:     "existing",
		Hostname:        "Api.Service",
		IP:              "10.0.0.2",
		OwnerPID:        owner.PID,
		OwnerStartTime:  owner.StartTime,
		GenerationAware: true,
	}}
	if _, _, err := entriesWithRegistration(entries, owner, "other", "api.service", "10.0.0.3", false); err == nil || !strings.Contains(err.Error(), "conflict") {
		t.Fatalf("case-folded registration error=%v, want live conflict", err)
	}
}

func TestValidateHostAndIPRejectsOverlongDNSName(t *testing.T) {
	label := strings.Repeat("a", 63)
	hostname := strings.Join([]string{label, label, label, label}, ".")
	if len(hostname) <= maxDNSHostnameBytes {
		t.Fatalf("test hostname length=%d, want > %d", len(hostname), maxDNSHostnameBytes)
	}
	if err := validateHostAndIP(hostname, "10.0.0.2"); err == nil || !strings.Contains(err.Error(), "DNS name limit") {
		t.Fatalf("overlong hostname error=%v, want DNS name length rejection", err)
	}
}

func TestValidateHostAndIPAllowsMaxLengthDNSName(t *testing.T) {
	hostname := strings.Join([]string{
		strings.Repeat("a", 63),
		strings.Repeat("b", 63),
		strings.Repeat("c", 63),
		strings.Repeat("d", 61),
	}, ".")
	if len(hostname) != maxDNSHostnameBytes {
		t.Fatalf("test hostname length=%d, want %d", len(hostname), maxDNSHostnameBytes)
	}
	if err := validateHostAndIP(hostname, "10.0.0.2"); err != nil {
		t.Fatalf("max-length hostname rejected: %v", err)
	}
}
