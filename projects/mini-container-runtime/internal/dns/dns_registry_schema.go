package dns

import (
	"bytes"
	"encoding/json"
	"fmt"
)

const currentDNSRegistrySchemaVersion = 1

type dnsRegistryEnvelope struct {
	SchemaVersion int         `json:"schema_version"`
	NetworkName   string      `json:"network_name"`
	Entries       []HostEntry `json:"entries"`
}

func decodeDNSRegistry(data []byte, expectedNetworkName string) ([]HostEntry, error) {
	trimmed := bytes.TrimSpace(data)
	if len(trimmed) == 0 {
		return nil, fmt.Errorf("empty DNS registry")
	}

	if trimmed[0] == '[' {
		var entries []HostEntry
		if err := json.Unmarshal(trimmed, &entries); err != nil {
			return nil, err
		}
		if err := validateDNSRegistryEntryCount(len(entries)); err != nil {
			return nil, err
		}
		if len(entries) != 0 {
			return nil, fmt.Errorf("non-empty historical DNS registry lacks network provenance; refusing authority for storage key %q", expectedNetworkName)
		}
		return entries, nil
	}
	if trimmed[0] != '{' {
		return nil, fmt.Errorf("DNS registry must be a versioned object or empty historical array")
	}

	var raw map[string]json.RawMessage
	if err := json.Unmarshal(trimmed, &raw); err != nil {
		return nil, err
	}
	rawVersion, ok := raw["schema_version"]
	if !ok {
		return nil, fmt.Errorf("DNS registry object missing schema version")
	}
	var version int
	if err := json.Unmarshal(rawVersion, &version); err != nil {
		return nil, fmt.Errorf("invalid DNS registry schema version: %w", err)
	}
	if version != currentDNSRegistrySchemaVersion {
		return nil, fmt.Errorf("unsupported DNS registry schema version %d", version)
	}

	rawNetwork, ok := raw["network_name"]
	if !ok {
		return nil, fmt.Errorf("DNS registry missing network provenance")
	}
	var networkName string
	if err := json.Unmarshal(rawNetwork, &networkName); err != nil {
		return nil, fmt.Errorf("invalid DNS registry network provenance: %w", err)
	}
	if networkName == "" || networkName != expectedNetworkName {
		return nil, fmt.Errorf("DNS registry network provenance %q does not match storage key %q", networkName, expectedNetworkName)
	}

	rawEntries, ok := raw["entries"]
	if !ok {
		return nil, fmt.Errorf("DNS registry missing entries")
	}
	entries, err := decodeCurrentRegistryEntries(rawEntries)
	if err != nil {
		return nil, err
	}
	return entries, nil
}

func decodeCurrentRegistryEntries(data []byte) ([]HostEntry, error) {
	var rawEntries []json.RawMessage
	if err := json.Unmarshal(data, &rawEntries); err != nil {
		return nil, fmt.Errorf("decode DNS registry entries: %w", err)
	}
	if err := validateDNSRegistryEntryCount(len(rawEntries)); err != nil {
		return nil, err
	}
	entries := make([]HostEntry, 0, len(rawEntries))
	for i, rawEntry := range rawEntries {
		var fields map[string]json.RawMessage
		if err := json.Unmarshal(rawEntry, &fields); err != nil {
			return nil, fmt.Errorf("decode DNS registry entry %d: %w", i, err)
		}
		rawVersion, ok := fields["schema_version"]
		if !ok {
			return nil, fmt.Errorf("DNS registry entry %d missing schema provenance", i)
		}
		var version int
		if err := json.Unmarshal(rawVersion, &version); err != nil {
			return nil, fmt.Errorf("invalid DNS registry entry %d schema version: %w", i, err)
		}
		if version != currentHostEntrySchemaVersion {
			return nil, fmt.Errorf("unsupported DNS registry entry %d schema version %d", i, version)
		}
		var entry HostEntry
		if err := json.Unmarshal(rawEntry, &entry); err != nil {
			return nil, fmt.Errorf("decode DNS registry entry %d: %w", i, err)
		}
		entries = append(entries, entry)
	}
	return entries, nil
}

func encodeDNSRegistry(networkName string, entries []HostEntry) ([]byte, error) {
	if err := validateDNSRegistryEntryCount(len(entries)); err != nil {
		return nil, err
	}
	return json.MarshalIndent(dnsRegistryEnvelope{
		SchemaVersion: currentDNSRegistrySchemaVersion,
		NetworkName:   networkName,
		Entries:       entries,
	}, "", "  ")
}
