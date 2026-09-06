package registry

import (
	"fmt"
	"net/http"
	"os"
	"path/filepath"

	"minicontainer/internal/state"
)

type MirrorServer struct {
	Port     int
	CacheDir string
}

func DefaultCacheDir() string {
	return filepath.Join(state.DefaultDir(), "registry-cache")
}

// StartMirrorServer initializes a local HTTP caching proxy server for OCI layer blobs.
func StartMirrorServer(port int, cacheDir string) (*http.Server, error) {
	if cacheDir == "" {
		cacheDir = DefaultCacheDir()
	}
	if err := os.MkdirAll(cacheDir, 0755); err != nil {
		return nil, fmt.Errorf("create cache dir: %w", err)
	}

	mux := http.NewServeMux()
	mux.HandleFunc("/v2/", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Docker-Distribution-Api-Version", "registry/2.0")
		w.Header().Set("Content-Type", "application/json")
		fmt.Fprintln(w, `{"status": "ok", "proxy": "minictl-mirror"}`)
	})

	srv := &http.Server{
		Addr:    fmt.Sprintf(":%d", port),
		Handler: mux,
	}

	return srv, nil
}
