// internal/registry/pull.go
//
// OCI Image Pulling (`minictl pull <image>`)
// Downloads and verifies every manifest layer before applying it locally.

package registry

import (
	"crypto/sha256"
	"crypto/subtle"
	"encoding/hex"
	"fmt"
	"io"
	"math"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"
)

const (
	defaultRegistryHost = "registry-1.docker.io"
	defaultAuthHost     = "auth.docker.io"
	manifestV2Header    = "application/vnd.docker.distribution.manifest.v2+json"
)

type Descriptor struct {
	MediaType string `json:"mediaType"`
	Size      int64  `json:"size"`
	Digest    string `json:"digest"`
}

type ManifestV2 struct {
	SchemaVersion int          `json:"schemaVersion"`
	Config        Descriptor   `json:"config"`
	Layers        []Descriptor `json:"layers"`
}

type authResponse struct {
	Token       string `json:"token"`
	AccessToken string `json:"access_token"`
}

// PullImage fetches an image from Docker Hub (e.g. "alpine" or "alpine:3.19")
// and extracts it to destDir. A destination that does not yet exist is assembled
// privately and published only after every verified layer applies successfully.
func PullImage(imageRef, destDir string) error {
	imageName, tag := parseImageRef(imageRef)
	if err := validateImageReference(imageName, tag); err != nil {
		return fmt.Errorf("invalid image reference %q: %w", imageRef, err)
	}
	fmt.Printf("Pulling image %s:%s from Docker Hub …\n", imageName, tag)

	token, err := getAuthToken(imageName)
	if err != nil {
		return fmt.Errorf("authentication failed: %w", err)
	}

	manifest, err := getManifest(imageName, tag, token)
	if err != nil {
		return fmt.Errorf("fetch manifest: %w", err)
	}
	if err := validateManifestLayers(manifest); err != nil {
		return fmt.Errorf("validate manifest: %w", err)
	}
	fmt.Printf("Image manifest loaded: %d layer(s)\n", len(manifest.Layers))

	tmpDir, err := os.MkdirTemp("", "minicontainer-pull-*")
	if err != nil {
		return fmt.Errorf("create temp dir: %w", err)
	}
	defer os.RemoveAll(tmpDir)

	client := &http.Client{Timeout: 60 * time.Second}
	runtimeConfig, err := pullImageRuntimeConfig(client, imageName, token, tmpDir, manifest.Config)
	if err != nil {
		return fmt.Errorf("fetch image config: %w", err)
	}

	layerFiles := make([]string, len(manifest.Layers))
	for i, layer := range manifest.Layers {
		short, err := shortDigest(layer.Digest)
		if err != nil {
			return fmt.Errorf("layer %d digest: %w", i+1, err)
		}
		fmt.Printf("  [%d/%d] downloading layer %s (%.2f MB) …\n",
			i+1, len(manifest.Layers), short, float64(layer.Size)/(1024*1024))

		layerFile := filepath.Join(tmpDir, fmt.Sprintf("layer-%d.tar.gz", i))
		if err := downloadBlob(client, imageName, layer.Digest, token, layerFile, layer.Size); err != nil {
			return fmt.Errorf("layer %d download failed: %w", i+1, err)
		}
		layerFiles[i] = layerFile
	}

	if err := applyVerifiedLayers(layerFiles, destDir); err != nil {
		return err
	}
	if err := persistPulledImageRuntimeMetadata(imageRef, destDir, runtimeConfig); err != nil {
		return fmt.Errorf("persist pulled image metadata: %w", err)
	}

	fmt.Printf("Successfully pulled %s:%s -> %s\n", imageName, tag, destDir)
	return nil
}

func parseImageRef(ref string) (string, string) {
	tag := "latest"
	name := ref
	if idx := strings.LastIndex(ref, ":"); idx != -1 && !strings.Contains(ref[idx:], "/") {
		name = ref[:idx]
		tag = ref[idx+1:]
	}
	if !strings.Contains(name, "/") {
		name = "library/" + name
	}
	return name, tag
}

func parseSHA256Digest(value string) ([]byte, error) {
	algorithm, encoded, ok := strings.Cut(value, ":")
	if !ok || algorithm != "sha256" {
		return nil, fmt.Errorf("unsupported or malformed digest %q", value)
	}
	if len(encoded) != sha256.Size*2 {
		return nil, fmt.Errorf("sha256 digest %q has invalid length", value)
	}
	digest, err := hex.DecodeString(encoded)
	if err != nil {
		return nil, fmt.Errorf("sha256 digest %q is not hexadecimal: %w", value, err)
	}
	if len(digest) != sha256.Size {
		return nil, fmt.Errorf("sha256 digest %q has invalid decoded length", value)
	}
	return digest, nil
}

func validateLayerDescriptor(layer Descriptor) error {
	if layer.Size < 0 || layer.Size == math.MaxInt64 {
		return fmt.Errorf("invalid layer size %d", layer.Size)
	}
	if _, err := parseSHA256Digest(layer.Digest); err != nil {
		return err
	}
	return nil
}

func validateManifestLayers(manifest *ManifestV2) error {
	if manifest == nil {
		return fmt.Errorf("manifest is nil")
	}
	if manifest.SchemaVersion != 2 {
		return fmt.Errorf("unsupported schema version %d", manifest.SchemaVersion)
	}
	if err := validateLayerDescriptor(manifest.Config); err != nil {
		return fmt.Errorf("config: %w", err)
	}
	if len(manifest.Layers) > maxManifestLayers {
		return fmt.Errorf("manifest contains %d layers; limit is %d", len(manifest.Layers), maxManifestLayers)
	}
	for i, layer := range manifest.Layers {
		if err := validateLayerDescriptor(layer); err != nil {
			return fmt.Errorf("layer %d: %w", i+1, err)
		}
	}
	return nil
}

func shortDigest(value string) (string, error) {
	if _, err := parseSHA256Digest(value); err != nil {
		return "", err
	}
	_, encoded, _ := strings.Cut(value, ":")
	return encoded[:12], nil
}

func getAuthToken(imageName string) (string, error) {
	endpoint, err := authTokenURL(imageName)
	if err != nil {
		return "", err
	}
	req, err := http.NewRequest(http.MethodGet, endpoint, nil)
	if err != nil {
		return "", err
	}
	resp, err := registryMetadataClient.Do(req)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("auth service returned status %d", resp.StatusCode)
	}
	var auth authResponse
	if err := decodeJSONLimited(resp.Body, maxAuthResponseBytes, &auth); err != nil {
		return "", err
	}
	return authTokenFromResponse(auth)
}

func getManifest(imageName, tag, token string) (*ManifestV2, error) {
	endpoint, err := manifestURL(imageName, tag)
	if err != nil {
		return nil, err
	}
	req, err := http.NewRequest(http.MethodGet, endpoint, nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("Accept", manifestV2Header)
	req.Header.Set("Authorization", "Bearer "+token)
	resp, err := registryMetadataClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("registry returned status %d", resp.StatusCode)
	}
	var manifest ManifestV2
	if err := decodeJSONLimited(resp.Body, maxManifestResponseBytes, &manifest); err != nil {
		return nil, err
	}
	return &manifest, nil
}

func writeVerifiedBlob(src io.Reader, destPath, digest string, expectedSize int64) error {
	expectedDigest, err := parseSHA256Digest(digest)
	if err != nil {
		return err
	}
	if expectedSize < 0 || expectedSize == math.MaxInt64 {
		return fmt.Errorf("invalid expected blob size %d", expectedSize)
	}

	out, err := os.OpenFile(destPath, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o600)
	if err != nil {
		return err
	}
	keep := false
	defer func() {
		_ = out.Close()
		if !keep {
			_ = os.Remove(destPath)
		}
	}()

	hasher := sha256.New()
	limited := io.LimitReader(src, expectedSize+1)
	written, err := io.Copy(io.MultiWriter(out, hasher), limited)
	if err != nil {
		return fmt.Errorf("write blob: %w", err)
	}
	if written != expectedSize {
		return fmt.Errorf("blob size mismatch: got %d bytes, want %d", written, expectedSize)
	}
	actualDigest := hasher.Sum(nil)
	if subtle.ConstantTimeCompare(actualDigest, expectedDigest) != 1 {
		return fmt.Errorf("blob digest mismatch: got sha256:%s, want %s", hex.EncodeToString(actualDigest), digest)
	}
	if err := out.Sync(); err != nil {
		return fmt.Errorf("sync verified blob: %w", err)
	}
	if err := out.Close(); err != nil {
		return fmt.Errorf("close verified blob: %w", err)
	}
	keep = true
	return nil
}

func downloadBlob(client *http.Client, imageName, digest, token, destPath string, expectedSize int64) error {
	if err := validateLayerDescriptor(Descriptor{Digest: digest, Size: expectedSize}); err != nil {
		return err
	}
	endpoint, err := blobURL(imageName, digest)
	if err != nil {
		return err
	}
	req, err := http.NewRequest(http.MethodGet, endpoint, nil)
	if err != nil {
		return err
	}
	req.Header.Set("Authorization", "Bearer "+token)
	resp, err := client.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("blob download status %d", resp.StatusCode)
	}
	if resp.ContentLength >= 0 && resp.ContentLength != expectedSize {
		return fmt.Errorf("blob content length mismatch: got %d bytes, want %d", resp.ContentLength, expectedSize)
	}
	return writeVerifiedBlob(resp.Body, destPath, digest, expectedSize)
}
