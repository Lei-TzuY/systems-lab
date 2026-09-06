package dns

import (
	"bytes"
	"strings"
	"testing"
)

func TestReadDNSRegistryContentsRejectsKnownOversizeBeforeReading(t *testing.T) {
	if _, err := readDNSRegistryContents(strings.NewReader("ignored"), maxDNSRegistryBytes+1, "default"); err == nil || !strings.Contains(err.Error(), "exceeds") {
		t.Fatalf("oversized registry error=%v, want size-limit rejection", err)
	}
}

func TestReadDNSRegistryContentsRejectsGrowthPastLimit(t *testing.T) {
	data := bytes.Repeat([]byte{'x'}, int(maxDNSRegistryBytes)+1)
	if _, err := readDNSRegistryContents(bytes.NewReader(data), 0, "default"); err == nil || !strings.Contains(err.Error(), "exceeds") {
		t.Fatalf("grown registry error=%v, want bounded-read rejection", err)
	}
}

func TestDNSRegistryDecodeRejectsEntryCountOverflow(t *testing.T) {
	var b strings.Builder
	b.WriteByte('[')
	for i := 0; i <= maxDNSRegistryEntries; i++ {
		if i > 0 {
			b.WriteByte(',')
		}
		b.WriteString("{\"schema_version\":1}")
	}
	b.WriteByte(']')

	if _, err := decodeCurrentRegistryEntries([]byte(b.String())); err == nil || !strings.Contains(err.Error(), "exceeds limit") {
		t.Fatalf("entry overflow error=%v, want cardinality rejection", err)
	}
}

func TestDNSRegistryEncodeRejectsEntryCountOverflow(t *testing.T) {
	entries := make([]HostEntry, maxDNSRegistryEntries+1)
	if _, err := encodeDNSRegistry("default", entries); err == nil || !strings.Contains(err.Error(), "exceeds limit") {
		t.Fatalf("encode overflow error=%v, want cardinality rejection", err)
	}
}
