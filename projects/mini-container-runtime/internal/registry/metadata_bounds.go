package registry

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"
)

const (
	maxAuthResponseBytes     int64 = 64 << 10
	maxManifestResponseBytes int64 = 8 << 20
	maxManifestLayers              = 1024
	registryMetadataTimeout         = 30 * time.Second
)

var registryMetadataClient = &http.Client{Timeout: registryMetadataTimeout}

func decodeJSONLimited(src io.Reader, limit int64, dst any) error {
	if src == nil {
		return fmt.Errorf("JSON response body is nil")
	}
	if limit <= 0 {
		return fmt.Errorf("JSON response limit must be positive")
	}
	data, err := io.ReadAll(io.LimitReader(src, limit+1))
	if err != nil {
		return fmt.Errorf("read JSON response: %w", err)
	}
	if int64(len(data)) > limit {
		return fmt.Errorf("JSON response exceeds %d-byte limit", limit)
	}
	if err := json.Unmarshal(data, dst); err != nil {
		return fmt.Errorf("decode JSON response: %w", err)
	}
	return nil
}

func authTokenFromResponse(auth authResponse) (string, error) {
	token := auth.Token
	if token == "" {
		token = auth.AccessToken
	}
	if strings.TrimSpace(token) == "" {
		return "", fmt.Errorf("auth service returned an empty token")
	}
	return token, nil
}
