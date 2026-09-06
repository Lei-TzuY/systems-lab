package registry

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"testing"

	"minicontainer/internal/state"
)

type roundTripFunc func(*http.Request) (*http.Response, error)

func (f roundTripFunc) RoundTrip(req *http.Request) (*http.Response, error) {
	return f(req)
}

func TestPullImageStopSignalDownloadsAndVerifiesConfigBlob(t *testing.T) {
	config := []byte(`{"config":{"StopSignal":"SIGUSR1"}}`)
	sum := sha256.Sum256(config)
	digest := "sha256:" + hex.EncodeToString(sum[:])

	client := &http.Client{Transport: roundTripFunc(func(req *http.Request) (*http.Response, error) {
		if got := req.Header.Get("Authorization"); got != "Bearer test-token" {
			t.Fatalf("Authorization=%q", got)
		}
		return &http.Response{
			StatusCode:    http.StatusOK,
			ContentLength: int64(len(config)),
			Body:          io.NopCloser(bytes.NewReader(config)),
			Header:        make(http.Header),
			Request:       req,
		}, nil
	})}

	signal, err := pullImageStopSignal(client, "library/demo", "test-token", t.TempDir(), Descriptor{
		Digest: digest,
		Size:   int64(len(config)),
	})
	if err != nil {
		t.Fatalf("pullImageStopSignal: %v", err)
	}
	if signal != "SIGUSR1" {
		t.Fatalf("signal=%q want SIGUSR1", signal)
	}
}

func TestPullImageStopSignalRejectsDigestMismatch(t *testing.T) {
	config := []byte(`{"config":{"StopSignal":"SIGUSR1"}}`)
	bad := sha256.Sum256([]byte("different config"))
	client := &http.Client{Transport: roundTripFunc(func(req *http.Request) (*http.Response, error) {
		return &http.Response{
			StatusCode:    http.StatusOK,
			ContentLength: int64(len(config)),
			Body:          io.NopCloser(bytes.NewReader(config)),
			Header:        make(http.Header),
			Request:       req,
		}, nil
	})}

	_, err := pullImageStopSignal(client, "library/demo", "test-token", t.TempDir(), Descriptor{
		Digest: "sha256:" + hex.EncodeToString(bad[:]),
		Size:   int64(len(config)),
	})
	if err == nil {
		t.Fatal("expected config digest mismatch")
	}
}

func TestParseImageConfigStopSignalDefaultsAndRejectsInvalidSignal(t *testing.T) {
	signal, err := parseImageConfigStopSignal([]byte(`{"config":{}}`))
	if err != nil {
		t.Fatalf("default StopSignal: %v", err)
	}
	if signal != "SIGTERM" {
		t.Fatalf("default signal=%q want SIGTERM", signal)
	}
	if _, err := parseImageConfigStopSignal([]byte(`{"config":{"StopSignal":"NOT_A_SIGNAL"}}`)); err == nil {
		t.Fatal("expected invalid StopSignal rejection")
	}
}

func TestPersistPulledImageMetadataRegistersStopSignal(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	rootfs := filepath.Join(t.TempDir(), "rootfs")
	if err := os.Mkdir(rootfs, 0o755); err != nil {
		t.Fatal(err)
	}

	if err := persistPulledImageMetadata("demo:latest", rootfs, "SIGUSR1"); err != nil {
		t.Fatalf("persistPulledImageMetadata: %v", err)
	}
	st, err := state.Open(state.DefaultDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	images, err := st.ListImages()
	if err != nil {
		t.Fatal(err)
	}
	found := false
	for _, img := range images {
		if img != nil && img.Name == "demo:latest" && img.RootFS == rootfs {
			found = true
			break
		}
	}
	if !found {
		t.Fatal("pulled image metadata was not registered")
	}
	signal, ok, err := st.ImageStopSignal("demo:latest")
	if err != nil {
		t.Fatal(err)
	}
	if !ok || signal != "SIGUSR1" {
		t.Fatalf("persisted StopSignal=(%q,%v) want (SIGUSR1,true)", signal, ok)
	}
}
