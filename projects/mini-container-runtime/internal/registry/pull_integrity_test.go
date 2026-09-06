package registry

import (
	"crypto/sha256"
	"encoding/hex"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func digestForTest(data []byte) string {
	sum := sha256.Sum256(data)
	return "sha256:" + hex.EncodeToString(sum[:])
}

func TestWriteVerifiedBlobAcceptsExactDigestAndSize(t *testing.T) {
	data := []byte("verified-layer-data")
	dest := filepath.Join(t.TempDir(), "layer.tar.gz")
	if err := writeVerifiedBlob(strings.NewReader(string(data)), dest, digestForTest(data), int64(len(data))); err != nil {
		t.Fatalf("writeVerifiedBlob: %v", err)
	}
	got, err := os.ReadFile(dest)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != string(data) {
		t.Fatalf("blob content=%q want=%q", got, data)
	}
	info, err := os.Stat(dest)
	if err != nil {
		t.Fatal(err)
	}
	if info.Mode().Perm() != 0o600 {
		t.Fatalf("verified blob mode=%#o", info.Mode().Perm())
	}
}

func TestWriteVerifiedBlobRejectsDigestMismatchAndRemovesOutput(t *testing.T) {
	data := []byte("actual-layer")
	dest := filepath.Join(t.TempDir(), "layer.tar.gz")
	err := writeVerifiedBlob(strings.NewReader(string(data)), dest, digestForTest([]byte("different-layer")), int64(len(data)))
	if err == nil || !strings.Contains(err.Error(), "digest mismatch") {
		t.Fatalf("digest mismatch error=%v", err)
	}
	if _, statErr := os.Stat(dest); !os.IsNotExist(statErr) {
		t.Fatalf("unverified blob left on disk: %v", statErr)
	}
}

func TestWriteVerifiedBlobRejectsSizeMismatchAndRemovesOutput(t *testing.T) {
	for _, tc := range []struct {
		name string
		data []byte
		size int64
	}{
		{name: "short", data: []byte("abc"), size: 4},
		{name: "long", data: []byte("abcd"), size: 3},
	} {
		t.Run(tc.name, func(t *testing.T) {
			dest := filepath.Join(t.TempDir(), "layer.tar.gz")
			err := writeVerifiedBlob(strings.NewReader(string(tc.data)), dest, digestForTest(tc.data), tc.size)
			if err == nil || !strings.Contains(err.Error(), "size mismatch") {
				t.Fatalf("size mismatch error=%v", err)
			}
			if _, statErr := os.Stat(dest); !os.IsNotExist(statErr) {
				t.Fatalf("wrong-size blob left on disk: %v", statErr)
			}
		})
	}
}

func TestWriteVerifiedBlobRejectsMalformedDigestBeforeCreatingOutput(t *testing.T) {
	dest := filepath.Join(t.TempDir(), "layer.tar.gz")
	err := writeVerifiedBlob(strings.NewReader("data"), dest, "sha256:abcd", 4)
	if err == nil || !strings.Contains(err.Error(), "invalid length") {
		t.Fatalf("malformed digest error=%v", err)
	}
	if _, statErr := os.Stat(dest); !os.IsNotExist(statErr) {
		t.Fatalf("malformed digest created output: %v", statErr)
	}
}

func TestWriteVerifiedBlobDoesNotOverwriteExistingDestination(t *testing.T) {
	data := []byte("new-layer")
	dest := filepath.Join(t.TempDir(), "layer.tar.gz")
	if err := os.WriteFile(dest, []byte("sentinel"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := writeVerifiedBlob(strings.NewReader(string(data)), dest, digestForTest(data), int64(len(data))); err == nil {
		t.Fatal("expected exclusive destination creation failure")
	}
	got, err := os.ReadFile(dest)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "sentinel" {
		t.Fatalf("existing destination overwritten: %q", got)
	}
}

func TestValidateManifestLayersPreflightsEveryDescriptor(t *testing.T) {
	config := []byte("{}")
	good := []byte("good-layer")
	manifest := &ManifestV2{
		SchemaVersion: 2,
		Config:        Descriptor{Digest: digestForTest(config), Size: int64(len(config))},
		Layers: []Descriptor{
			{Digest: digestForTest(good), Size: int64(len(good))},
			{Digest: "sha256:too-short", Size: 10},
		},
	}
	if err := validateManifestLayers(manifest); err == nil || !strings.Contains(err.Error(), "layer 2") {
		t.Fatalf("manifest preflight error=%v", err)
	}
}

func TestShortDigestRejectsMalformedInputInsteadOfPanicking(t *testing.T) {
	if _, err := shortDigest("bad"); err == nil {
		t.Fatal("expected malformed short digest to be rejected")
	}
}
