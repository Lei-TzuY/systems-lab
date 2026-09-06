//go:build linux

package dns

import (
	"os"
	"strings"
	"testing"
)

func TestGenerateHostsHidesPendingRuntimeAdmission(t *testing.T) {
	useTempDNSHome(t)
	rollback, err := BeginHostRegistrationAttempt("default", "ctr-pending", "pending-host", "10.0.0.20")
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = rollback() }()

	content, err := GenerateHostsContentChecked("default")
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(content, "pending-host") {
		t.Fatalf("pending admission leaked into service discovery:\n%s", content)
	}

	entry := readSingleHostEntry(t, "default")
	if !entry.GenerationAware || !entry.AdmissionPending || entry.GenerationPID != 0 || entry.GenerationStartTime != 0 {
		t.Fatalf("hidden reservation has wrong durable state: %+v", entry)
	}
}

func TestPendingRuntimeAdmissionStillRejectsCompetingHostname(t *testing.T) {
	useTempDNSHome(t)
	rollback, err := BeginHostRegistrationAttempt("default", "ctr-owner", "reserved-host", "10.0.0.21")
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = rollback() }()

	if err := RegisterHost("default", "ctr-racer", "reserved-host", "10.0.0.22"); err == nil {
		t.Fatal("competing registration stole a pending hostname reservation")
	}
	content, err := GenerateHostsContentChecked("default")
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(content, "reserved-host") {
		t.Fatalf("reserved but pending hostname became discoverable:\n%s", content)
	}
}

func TestExactGenerationBindPublishesPendingRuntimeAdmission(t *testing.T) {
	useTempDNSHome(t)
	identity, err := currentRegistrarIdentity()
	if err != nil {
		t.Fatal(err)
	}
	rollback, err := BeginHostRegistrationAttempt("default", "ctr-ready", "ready-host", "10.0.0.23")
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = rollback() }()

	if err := BindHostRegistrationGeneration("default", "ctr-ready", os.Getpid(), identity.StartTime); err != nil {
		t.Fatal(err)
	}
	entry := readSingleHostEntry(t, "default")
	if entry.AdmissionPending || entry.GenerationPID != os.Getpid() || entry.GenerationStartTime != identity.StartTime {
		t.Fatalf("bind did not atomically publish exact generation: %+v", entry)
	}

	content, err := GenerateHostsContentChecked("default")
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(content, "10.0.0.23\tready-host") {
		t.Fatalf("bound generation was not published to service discovery:\n%s", content)
	}
}
