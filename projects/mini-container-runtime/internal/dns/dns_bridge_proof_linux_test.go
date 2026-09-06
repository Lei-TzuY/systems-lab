//go:build linux

package dns

import (
	"os"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func setupRunningRulesOnlyDNSOwner(t *testing.T, containerID, hostname, mappingIP string) registrarIdentity {
	t.Helper()
	identity, err := currentRegistrarIdentity()
	if err != nil {
		t.Fatal(err)
	}
	st, err := state.Open(state.DefaultDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&state.Container{
		ID:        containerID,
		Status:    state.StatusCreated,
		RootFS:    "/tmp/rootfs",
		Command:   []string{"true"},
		Hostname:  hostname,
		CreatedAt: time.Now(),
	}); err != nil {
		_ = st.Close()
		t.Fatal(err)
	}
	if err := st.MarkRunning(containerID, os.Getpid(), identity.StartTime, time.Now()); err != nil {
		_ = st.Close()
		t.Fatal(err)
	}
	if err := st.MarkNetworkOwnedIfIdentity(containerID, state.NetworkOwnership{
		Owner:        "minicontainer:dns-rules-only-test",
		PID:          os.Getpid(),
		PIDStartTime: identity.StartTime,
		Mappings: []state.PortForwardingOwnership{{
			HostPort:      18080,
			ContainerPort: 8080,
			ContainerIP:   mappingIP,
			Protocol:      "tcp",
		}},
	}); err != nil {
		_ = st.Close()
		t.Fatalf("persist rules-only ownership: %v", err)
	}
	if err := st.Close(); err != nil {
		t.Fatal(err)
	}
	return identity
}

func TestDNSDeadRegistrarRulesOnlyOwnershipMustMatchEntryIP(t *testing.T) {
	useTempDNSHome(t)
	const containerID = "rules-only-dns-owner"
	setupRunningRulesOnlyDNSOwner(t, containerID, "rules-only-host", "172.20.0.99")

	saveDeadRegistrarEntry(t, HostEntry{
		ContainerID:    containerID,
		Hostname:       "rules-only-host",
		IP:             "172.20.0.2",
		OwnerPID:       99999999,
		OwnerStartTime: 1,
	})

	content, err := GenerateHostsContentChecked("default")
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(content, "172.20.0.2\trules-only-host") {
		t.Fatalf("unrelated rules-only ownership incorrectly adopted DNS entry:\n%s", content)
	}
}

func TestDNSDeadRegistrarMatchingRulesOnlyOwnershipRemainsCompatible(t *testing.T) {
	useTempDNSHome(t)
	const containerID = "legacy-rules-only-dns-owner"
	setupRunningRulesOnlyDNSOwner(t, containerID, "legacy-rules-host", "172.20.0.2")

	saveDeadRegistrarEntry(t, HostEntry{
		ContainerID:    containerID,
		Hostname:       "legacy-rules-host",
		IP:             "172.20.0.2",
		OwnerPID:       99999999,
		OwnerStartTime: 1,
	})

	content, err := GenerateHostsContentChecked("default")
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(content, "172.20.0.2\tlegacy-rules-host") {
		t.Fatalf("matching legacy rules-only ownership was not adopted:\n%s", content)
	}
}
