package registry

import (
	"strings"
	"testing"
)

type endlessMetadataReader struct {
	read int
}

func (r *endlessMetadataReader) Read(p []byte) (int, error) {
	for i := range p {
		p[i] = 'x'
	}
	r.read += len(p)
	return len(p), nil
}

func TestDecodeJSONLimitedStopsAfterLimitPlusOne(t *testing.T) {
	reader := &endlessMetadataReader{}
	var dst map[string]any
	const limit int64 = 32
	err := decodeJSONLimited(reader, limit, &dst)
	if err == nil || !strings.Contains(err.Error(), "exceeds") {
		t.Fatalf("oversized JSON error=%v", err)
	}
	if reader.read != int(limit+1) {
		t.Fatalf("decoder read=%d bytes, want exactly %d", reader.read, limit+1)
	}
}

func TestDecodeJSONLimitedAcceptsResponseAtLimit(t *testing.T) {
	body := `{"token":"abc"}`
	var auth authResponse
	if err := decodeJSONLimited(strings.NewReader(body), int64(len(body)), &auth); err != nil {
		t.Fatalf("decode exact-limit JSON: %v", err)
	}
	if auth.Token != "abc" {
		t.Fatalf("token=%q", auth.Token)
	}
}

func TestAuthTokenFromResponseRequiresNonEmptyToken(t *testing.T) {
	if token, err := authTokenFromResponse(authResponse{Token: "primary", AccessToken: "fallback"}); err != nil || token != "primary" {
		t.Fatalf("primary token=%q err=%v", token, err)
	}
	if token, err := authTokenFromResponse(authResponse{AccessToken: "fallback"}); err != nil || token != "fallback" {
		t.Fatalf("fallback token=%q err=%v", token, err)
	}
	if _, err := authTokenFromResponse(authResponse{}); err == nil || !strings.Contains(err.Error(), "empty token") {
		t.Fatalf("empty auth token error=%v", err)
	}
	if _, err := authTokenFromResponse(authResponse{Token: "   "}); err == nil || !strings.Contains(err.Error(), "empty token") {
		t.Fatalf("whitespace auth token error=%v", err)
	}
}

func TestRegistryMetadataClientHasHardTimeout(t *testing.T) {
	if registryMetadataClient == nil {
		t.Fatal("registry metadata client is nil")
	}
	if registryMetadataClient.Timeout != registryMetadataTimeout || registryMetadataClient.Timeout <= 0 {
		t.Fatalf("metadata timeout=%v want=%v", registryMetadataClient.Timeout, registryMetadataTimeout)
	}
}

func TestValidateManifestLayersRejectsExcessiveLayerCount(t *testing.T) {
	config := []byte("{}")
	manifest := &ManifestV2{
		SchemaVersion: 2,
		Config:        Descriptor{Digest: digestForTest(config), Size: int64(len(config))},
		Layers:        make([]Descriptor, maxManifestLayers+1),
	}
	err := validateManifestLayers(manifest)
	if err == nil || !strings.Contains(err.Error(), "limit") {
		t.Fatalf("excessive layer count error=%v", err)
	}
}

func TestMetadataResponseLimitsAreFinite(t *testing.T) {
	if maxAuthResponseBytes <= 0 || maxManifestResponseBytes <= 0 {
		t.Fatalf("invalid response limits: auth=%d manifest=%d", maxAuthResponseBytes, maxManifestResponseBytes)
	}
	if maxAuthResponseBytes >= maxManifestResponseBytes {
		t.Fatalf("auth response limit %d should be smaller than manifest limit %d", maxAuthResponseBytes, maxManifestResponseBytes)
	}
}
