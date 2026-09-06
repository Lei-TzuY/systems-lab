//go:build linux

package container

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
	"time"

	"minicontainer/internal/dns"
	"minicontainer/internal/state"
)

func setupStoppedDNSRecoveryState(t *testing.T, id string) (*state.Store, *state.Container) {
	t.Helper()
	t.Setenv("HOME", t.TempDir())
	st, err := state.Open(state.DefaultDir())
	if err != nil {
		t.Fatal(err)
	}
	c := &state.Container{
		ID:        id,
		Status:    state.StatusStopped,
		RootFS:    "/tmp/rootfs",
		Command:   []string{"true"},
		Hostname:  id + "-host",
		CreatedAt: time.Now(),
	}
	if err := st.Save(c); err != nil {
		_ = st.Close()
		t.Fatal(err)
	}
	persisted, err := st.Get(id)
	if err != nil {
		_ = st.Close()
		t.Fatal(err)
	}
	return st, persisted
}

func writeDNSRecoveryEntries(t *testing.T, entries []dns.HostEntry) string {
	t.Helper()
	dir := dns.DefaultDNSDir()
	if err := os.MkdirAll(dir, 0o700); err != nil {
		t.Fatal(err)
	}
	data, err := json.MarshalIndent(struct {
		SchemaVersion int             `json:"schema_version"`
		NetworkName   string          `json:"network_name"`
		Entries       []dns.HostEntry `json:"entries"`
	}{
		SchemaVersion: 1,
		NetworkName:   defaultBridgeDNSNetwork,
		Entries:       entries,
	}, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(dir, defaultBridgeDNSNetwork+".json")
	if err := os.WriteFile(path, data, 0o600); err != nil {
		t.Fatal(err)
	}
	return path
}

func readDNSRecoveryEntries(t *testing.T, path string) []dns.HostEntry {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	trimmed := bytes.TrimSpace(data)
	var entries []dns.HostEntry
	if len(trimmed) > 0 && trimmed[0] == '{' {
		var envelope struct {
			Entries []dns.HostEntry `json:"entries"`
		}
		if err := json.Unmarshal(trimmed, &envelope); err != nil {
			t.Fatal(err)
		}
		entries = envelope.Entries
	} else if err := json.Unmarshal(trimmed, &entries); err != nil {
		t.Fatal(err)
	}
	return entries
}

func TestCleanupStoppedRuntimeResourcesRetriesStaleDNS(t *testing.T) {
	const id = "stopped-stale-dns"
	st, c := setupStoppedDNSRecoveryState(t, id)
	defer st.Close()

	path := writeDNSRecoveryEntries(t, []dns.HostEntry{{
		ContainerID:    id,
		Hostname:       c.Hostname,
		IP:             "172.20.0.2",
		OwnerPID:       99999999,
		OwnerStartTime: 1,
	}})

	if err := CleanupStoppedRuntimeResources(st, c); err != nil {
		t.Fatalf("cleanup stopped runtime resources: %v", err)
	}
	if got := readDNSRecoveryEntries(t, path); len(got) != 0 {
		t.Fatalf("stale DNS registration survived stopped recovery: %+v", got)
	}
}

func TestCleanupStoppedRuntimeResourcesPreservesCurrentRegistrarDNS(t *testing.T) {
	const id = "stopped-current-dns"
	st, c := setupStoppedDNSRecoveryState(t, id)
	defer st.Close()

	if err := dns.RegisterHost(defaultBridgeDNSNetwork, id, c.Hostname, "172.20.0.2"); err != nil {
		t.Fatalf("register current DNS owner: %v", err)
	}
	path := filepath.Join(dns.DefaultDNSDir(), defaultBridgeDNSNetwork+".json")

	if err := CleanupStoppedRuntimeResources(st, c); err != nil {
		t.Fatalf("cleanup stopped runtime resources: %v", err)
	}
	got := readDNSRecoveryEntries(t, path)
	if len(got) != 1 || got[0].ContainerID != id {
		t.Fatalf("retry cleanup consumed current registrar registration: %+v", got)
	}
}
