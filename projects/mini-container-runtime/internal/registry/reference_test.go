package registry

import (
	"net/url"
	"strings"
	"testing"
)

func TestValidateImageReferenceAcceptsDockerHubSubset(t *testing.T) {
	valid := []struct {
		name string
		tag  string
	}{
		{name: "library/alpine", tag: "latest"},
		{name: "myorg/myapp", tag: "v1.2.3"},
		{name: "org/repo__name", tag: "Build_42"},
		{name: "org/repo--name", tag: "release-2026.08"},
		{name: "a/b.c_d-e", tag: "A"},
	}
	for _, tc := range valid {
		if err := validateImageReference(tc.name, tc.tag); err != nil {
			t.Errorf("validateImageReference(%q,%q): %v", tc.name, tc.tag, err)
		}
	}
}

func TestValidateImageReferenceRejectsRequestConfusionInputs(t *testing.T) {
	invalid := []struct {
		name string
		tag  string
	}{
		{name: "", tag: "latest"},
		{name: "library/../admin", tag: "latest"},
		{name: "library/alpine?scope=admin", tag: "latest"},
		{name: "library/alpine#fragment", tag: "latest"},
		{name: "library/alpine%2fadmin", tag: "latest"},
		{name: "Library/alpine", tag: "latest"},
		{name: "localhost:5000/alpine", tag: "latest"},
		{name: "library/alpine@sha256", tag: "deadbeef"},
		{name: "library/alpine", tag: ""},
		{name: "library/alpine", tag: "../latest"},
		{name: "library/alpine", tag: "latest?x=y"},
		{name: "library/alpine", tag: "latest#fragment"},
		{name: "library/alpine", tag: strings.Repeat("a", 129)},
	}
	for _, tc := range invalid {
		if err := validateImageReference(tc.name, tc.tag); err == nil {
			t.Errorf("validateImageReference(%q,%q) unexpectedly succeeded", tc.name, tc.tag)
		}
	}
}

func TestAuthTokenURLUsesSingleEncodedScopeValue(t *testing.T) {
	endpoint, err := authTokenURL("library/alpine")
	if err != nil {
		t.Fatal(err)
	}
	u, err := url.Parse(endpoint)
	if err != nil {
		t.Fatal(err)
	}
	if u.Scheme != "https" || u.Host != defaultAuthHost || u.Path != "/token" {
		t.Fatalf("auth URL=%q", endpoint)
	}
	query := u.Query()
	if got := query.Get("service"); got != "registry.docker.io" {
		t.Fatalf("service=%q", got)
	}
	if got := query["scope"]; len(got) != 1 || got[0] != "repository:library/alpine:pull" {
		t.Fatalf("scope=%v", got)
	}
	if _, err := authTokenURL("library/alpine&service=evil"); err == nil {
		t.Fatal("expected auth query injection input to be rejected")
	}
}

func TestManifestAndBlobURLsStayOnRegistryHost(t *testing.T) {
	manifestEndpoint, err := manifestURL("library/alpine", "3.19")
	if err != nil {
		t.Fatal(err)
	}
	manifestParsed, err := url.Parse(manifestEndpoint)
	if err != nil {
		t.Fatal(err)
	}
	if manifestParsed.Host != defaultRegistryHost || manifestParsed.Path != "/v2/library/alpine/manifests/3.19" || manifestParsed.RawQuery != "" {
		t.Fatalf("manifest URL=%q", manifestEndpoint)
	}

	digest := digestForTest([]byte("layer"))
	blobEndpoint, err := blobURL("library/alpine", digest)
	if err != nil {
		t.Fatal(err)
	}
	blobParsed, err := url.Parse(blobEndpoint)
	if err != nil {
		t.Fatal(err)
	}
	if blobParsed.Host != defaultRegistryHost || blobParsed.RawQuery != "" {
		t.Fatalf("blob URL=%q", blobEndpoint)
	}
	if !strings.HasSuffix(blobParsed.Path, "/blobs/"+digest) {
		t.Fatalf("blob path=%q", blobParsed.Path)
	}
	if _, err := blobURL("library/alpine", "sha256:bad?query"); err == nil {
		t.Fatal("expected malformed blob digest to be rejected")
	}
}

func TestPullImageRejectsInvalidReferenceBeforeNetwork(t *testing.T) {
	err := PullImage("alpine:latest?scope=admin", t.TempDir())
	if err == nil || !strings.Contains(err.Error(), "invalid image reference") {
		t.Fatalf("PullImage invalid reference error=%v", err)
	}
}
