package dns

import (
	"encoding/json"
	"strings"
	"testing"
)

func TestHostEntryMarshalPersistsCurrentSchemaVersion(t *testing.T) {
	entry := HostEntry{
		ContainerID:         "container-a",
		Hostname:            "host-a",
		IP:                  "10.0.0.2",
		OwnerPID:            101,
		OwnerStartTime:      202,
		GenerationAware:     true,
		GenerationPID:       303,
		GenerationStartTime: 404,
	}

	data, err := json.Marshal(entry)
	if err != nil {
		t.Fatal(err)
	}
	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		t.Fatal(err)
	}
	var version int
	if err := json.Unmarshal(raw["schema_version"], &version); err != nil {
		t.Fatalf("decode schema version from %s: %v", data, err)
	}
	if version != currentHostEntrySchemaVersion {
		t.Fatalf("schema_version=%d, want %d; data=%s", version, currentHostEntrySchemaVersion, data)
	}
	if string(raw["container_id"]) != `"container-a"` {
		t.Fatalf("container provenance missing from versioned record: %s", data)
	}
	if string(raw["generation_pid"]) != "303" || string(raw["generation_start_time"]) != "404" {
		t.Fatalf("generation provenance missing from versioned record: %s", data)
	}
}

func TestHostEntryUnmarshalPreservesPreSchemaCompatibility(t *testing.T) {
	const historical = `{"container_id":"legacy","hostname":"legacy-host","ip":"10.0.0.3","owner_pid":11,"owner_start_time":22,"generation_aware":true}`
	var entry HostEntry
	if err := json.Unmarshal([]byte(historical), &entry); err != nil {
		t.Fatalf("pre-schema host entry rejected: %v", err)
	}
	if entry.ContainerID != "legacy" || entry.OwnerPID != 11 || entry.OwnerStartTime != 22 || !entry.GenerationAware {
		t.Fatalf("decoded historical entry=%+v", entry)
	}
}

func TestHostEntryUnmarshalRejectsUnsupportedSchemaVersions(t *testing.T) {
	for _, tc := range []struct {
		name    string
		version string
	}{
		{name: "zero", version: "0"},
		{name: "future", version: "2"},
		{name: "null", version: "null"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			data := `{"schema_version":` + tc.version + `,"container_id":"victim","hostname":"victim-host","ip":"10.0.0.4"}`
			var entry HostEntry
			err := json.Unmarshal([]byte(data), &entry)
			if err == nil || !strings.Contains(err.Error(), "schema version") {
				t.Fatalf("schema %s error=%v", tc.version, err)
			}
		})
	}
}

func TestHostEntryUnmarshalRejectsMalformedSchemaVersion(t *testing.T) {
	const malformed = `{"schema_version":"1","container_id":"victim","hostname":"victim-host","ip":"10.0.0.4"}`
	var entry HostEntry
	err := json.Unmarshal([]byte(malformed), &entry)
	if err == nil || !strings.Contains(err.Error(), "invalid DNS host entry schema version") {
		t.Fatalf("malformed schema error=%v", err)
	}
}

func TestVersionedHostEntryRoundTripPreservesOwnership(t *testing.T) {
	want := HostEntry{
		ContainerID:         "container-b",
		Hostname:            "host-b",
		IP:                  "10.0.0.5",
		OwnerPID:            111,
		OwnerStartTime:      222,
		GenerationAware:     true,
		GenerationPID:       333,
		GenerationStartTime: 444,
		AdmissionPending:    true,
	}
	data, err := json.Marshal(want)
	if err != nil {
		t.Fatal(err)
	}
	var got HostEntry
	if err := json.Unmarshal(data, &got); err != nil {
		t.Fatal(err)
	}
	if got != want {
		t.Fatalf("round trip got=%+v want=%+v data=%s", got, want, data)
	}
}
