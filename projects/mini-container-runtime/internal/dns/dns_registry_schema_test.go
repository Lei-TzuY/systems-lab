package dns

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestDNSRegistryEncodeBindsNetworkProvenance(t *testing.T) {
	entries := []HostEntry{{ContainerID: "c1", Hostname: "host-a", IP: "10.0.0.2"}}
	data, err := encodeDNSRegistry("net-a", entries)
	if err != nil {
		t.Fatal(err)
	}
	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		t.Fatal(err)
	}
	if string(raw["schema_version"]) != "1" {
		t.Fatalf("schema_version=%s data=%s", raw["schema_version"], data)
	}
	if string(raw["network_name"]) != `"net-a"` {
		t.Fatalf("network provenance missing: %s", data)
	}
	if len(raw["entries"]) == 0 {
		t.Fatalf("entries missing: %s", data)
	}
}

func TestDNSRegistryDecodeRejectsMovedVersionedRegistry(t *testing.T) {
	data, err := encodeDNSRegistry("net-a", []HostEntry{{ContainerID: "c1", Hostname: "host-a", IP: "10.0.0.2"}})
	if err != nil {
		t.Fatal(err)
	}
	_, err = decodeDNSRegistry(data, "net-b")
	if err == nil || !strings.Contains(err.Error(), "does not match storage key") {
		t.Fatalf("moved registry error=%v", err)
	}
}

func TestDNSRegistryDecodeRejectsInvalidEnvelopeProvenance(t *testing.T) {
	for _, tc := range []struct {
		name string
		data string
	}{
		{name: "future schema", data: `{"schema_version":2,"network_name":"net-a","entries":[]}`},
		{name: "zero schema", data: `{"schema_version":0,"network_name":"net-a","entries":[]}`},
		{name: "null schema", data: `{"schema_version":null,"network_name":"net-a","entries":[]}`},
		{name: "missing schema", data: `{"network_name":"net-a","entries":[]}`},
		{name: "missing network", data: `{"schema_version":1,"entries":[]}`},
		{name: "missing entries", data: `{"schema_version":1,"network_name":"net-a"}`},
	} {
		t.Run(tc.name, func(t *testing.T) {
			if _, err := decodeDNSRegistry([]byte(tc.data), "net-a"); err == nil {
				t.Fatalf("decode unexpectedly accepted %s", tc.data)
			}
		})
	}
}

func TestDNSRegistryDecodeRejectsDowngradedEntryProvenance(t *testing.T) {
	for _, tc := range []struct {
		name string
		entry string
	}{
		{name: "missing schema", entry: `{"container_id":"c1","hostname":"host-a","ip":"10.0.0.2"}`},
		{name: "zero schema", entry: `{"schema_version":0,"container_id":"c1","hostname":"host-a","ip":"10.0.0.2"}`},
		{name: "future schema", entry: `{"schema_version":2,"container_id":"c1","hostname":"host-a","ip":"10.0.0.2"}`},
		{name: "null schema", entry: `{"schema_version":null,"container_id":"c1","hostname":"host-a","ip":"10.0.0.2"}`},
	} {
		t.Run(tc.name, func(t *testing.T) {
			data := []byte(`{"schema_version":1,"network_name":"net-a","entries":[` + tc.entry + `]}`)
			if _, err := decodeDNSRegistry(data, "net-a"); err == nil {
				t.Fatalf("current registry unexpectedly accepted downgraded entry: %s", data)
			}
		})
	}
}

func TestDNSRegistryDecodeAcceptsCurrentEntryProvenance(t *testing.T) {
	data, err := encodeDNSRegistry("net-a", []HostEntry{{
		ContainerID:         "c1",
		Hostname:            "host-a",
		IP:                  "10.0.0.2",
		OwnerPID:            123,
		OwnerStartTime:      456,
		GenerationAware:     true,
		GenerationPID:       789,
		GenerationStartTime: 987,
	}})
	if err != nil {
		t.Fatal(err)
	}
	entries, err := decodeDNSRegistry(data, "net-a")
	if err != nil {
		t.Fatalf("current registry rejected: %v", err)
	}
	if len(entries) != 1 || entries[0].ContainerID != "c1" || entries[0].GenerationPID != 789 {
		t.Fatalf("decoded entries=%+v", entries)
	}
}

func TestDNSRegistryDecodeAllowsOnlyAuthorityFreeHistoricalArray(t *testing.T) {
	entries, err := decodeDNSRegistry([]byte(`[]`), "net-a")
	if err != nil {
		t.Fatalf("empty historical registry rejected: %v", err)
	}
	if len(entries) != 0 {
		t.Fatalf("decoded entries=%+v", entries)
	}
}

func TestDNSRegistryDecodeRejectsNonEmptyHistoricalArrayWithoutNetworkProvenance(t *testing.T) {
	for _, historical := range []string{
		`[{"container_id":"legacy","hostname":"legacy-host","ip":"10.0.0.3"}]`,
		`[{"schema_version":1,"container_id":"transitional","hostname":"host-a","ip":"10.0.0.4","owner_pid":123,"owner_start_time":456}]`,
	} {
		if _, err := decodeDNSRegistry([]byte(historical), "net-b"); err == nil || !strings.Contains(err.Error(), "lacks network provenance") {
			t.Fatalf("unbound historical registry unexpectedly authoritative: data=%s err=%v", historical, err)
		}
	}
}

func TestSaveEntriesAtomicUpgradesRegistryEnvelopeAndLoadChecksKey(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "net-a.json")
	entries := []HostEntry{{ContainerID: "c1", Hostname: "host-a", IP: "10.0.0.2"}}
	if err := saveEntriesAtomic(dir, path, "net-a", entries); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(data), `"network_name": "net-a"`) {
		t.Fatalf("versioned registry provenance missing: %s", data)
	}
	if !strings.Contains(string(data), `"schema_version": 1`) {
		t.Fatalf("entry schema provenance missing: %s", data)
	}
	got, exists, err := loadEntriesChecked(path, "net-a")
	if err != nil || !exists || len(got) != 1 || got[0].ContainerID != "c1" {
		t.Fatalf("load got=%+v exists=%v err=%v", got, exists, err)
	}
	if _, _, err := loadEntriesChecked(path, "net-b"); err == nil || !strings.Contains(err.Error(), "does not match storage key") {
		t.Fatalf("cross-network load error=%v", err)
	}
}
