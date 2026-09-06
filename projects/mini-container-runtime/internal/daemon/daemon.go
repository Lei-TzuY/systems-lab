package daemon

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	"minicontainer/internal/imagestore"
	"minicontainer/internal/metrics"
	"minicontainer/internal/state"
	runtimestats "minicontainer/internal/stats"
)

const (
	defaultListenAddr = "unix:///tmp/minictl.sock"
	unixSocketMode    = 0o600
)

// Server represents the minictl REST API Daemon.
type Server struct {
	addr       string
	network    string
	listener   net.Listener
	httpServer *http.Server
	store      *state.Store
	socketInfo os.FileInfo
}

// Config options for starting daemon server.
type Config struct {
	ListenAddr string // e.g. "unix:///tmp/minictl.sock" or "tcp://127.0.0.1:2375"
	StoreDir   string
}

// NewServer initializes daemon server.
func NewServer(cfg Config) (*Server, error) {
	stDir := cfg.StoreDir
	if stDir == "" {
		stDir = state.DefaultDir()
	}

	st, err := state.Open(stDir)
	if err != nil {
		return nil, fmt.Errorf("open state store: %w", err)
	}
	storeOwned := true
	defer func() {
		if storeOwned {
			_ = st.Close()
		}
	}()

	network, listenPath, err := resolveListenAddress(cfg.ListenAddr)
	if err != nil {
		return nil, err
	}
	if network == "unix" {
		if err := ensureUnixSocketPathAvailable(listenPath); err != nil {
			return nil, err
		}
	}

	l, err := listen(network, listenPath)
	if err != nil {
		return nil, fmt.Errorf("listen %s %s: %w", network, listenPath, err)
	}

	var socketInfo os.FileInfo
	if network == "unix" {
		unixListener, ok := l.(*net.UnixListener)
		if !ok {
			_ = l.Close()
			return nil, fmt.Errorf("unix listener has unexpected type %T", l)
		}
		// Go's UnixListener normally unlinks its path on Close. Disable that
		// behavior so cleanup can verify the path still refers to the exact
		// socket inode created here instead of deleting a replacement path.
		unixListener.SetUnlinkOnClose(false)

		socketInfo, err = os.Lstat(listenPath)
		if err != nil {
			_ = l.Close()
			return nil, fmt.Errorf("stat newly created unix socket %s: %w", listenPath, err)
		}
		if socketInfo.Mode()&os.ModeSocket == 0 {
			_ = l.Close()
			return nil, fmt.Errorf("new unix listener path %s is not a socket", listenPath)
		}
		if got := socketInfo.Mode().Perm(); got != unixSocketMode {
			_ = l.Close()
			_ = removeUnixSocketIfSame(listenPath, socketInfo)
			return nil, fmt.Errorf("unix socket %s permissions are %o, want %o", listenPath, got, unixSocketMode)
		}
	}

	srv := &Server{
		addr:       listenPath,
		network:    network,
		listener:   l,
		store:      st,
		socketInfo: socketInfo,
	}

	mux := http.NewServeMux()
	mux.HandleFunc("/v1/system/info", srv.handleSystemInfo)
	mux.HandleFunc("/v1/containers/json", srv.handleListContainers)
	mux.HandleFunc("/v1/containers/", srv.handleContainerRoute)
	mux.HandleFunc("/v1/images/json", srv.handleListImages)
	mux.HandleFunc("/v1/stats", srv.handleStats)
	mux.HandleFunc("/v1/metrics", srv.handleMetrics)

	srv.httpServer = &http.Server{
		Handler:           mux,
		ReadHeaderTimeout: 5 * time.Second,
		ReadTimeout:       10 * time.Second,
		WriteTimeout:      10 * time.Second,
		IdleTimeout:       60 * time.Second,
		MaxHeaderBytes:    64 << 10,
	}

	storeOwned = false
	return srv, nil
}

func resolveListenAddress(raw string) (network, address string, err error) {
	if raw == "" {
		raw = defaultListenAddr
	}

	switch {
	case strings.HasPrefix(raw, "unix://"):
		address = strings.TrimPrefix(raw, "unix://")
		if address == "" {
			return "", "", fmt.Errorf("unix listen path must not be empty")
		}
		if !filepath.IsAbs(address) {
			return "", "", fmt.Errorf("unix listen path must be absolute: %q", address)
		}
		return "unix", address, nil
	case strings.HasPrefix(raw, "tcp://"):
		address = strings.TrimPrefix(raw, "tcp://")
		if address == "" {
			return "", "", fmt.Errorf("TCP listen address must not be empty")
		}
		return "tcp", address, nil
	case strings.Contains(raw, "://"):
		return "", "", fmt.Errorf("unsupported listen address scheme in %q", raw)
	default:
		// Preserve compatibility with bare host:port TCP addresses.
		return "tcp", raw, nil
	}
}

func ensureUnixSocketPathAvailable(path string) error {
	info, err := os.Lstat(path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil
		}
		return fmt.Errorf("inspect unix socket path %s: %w", path, err)
	}

	if info.Mode()&os.ModeSocket != 0 {
		return fmt.Errorf("unix socket path %s already exists; refusing to replace it", path)
	}
	return fmt.Errorf("refusing to remove non-socket path %s (mode %s)", path, info.Mode())
}

func removeUnixSocketIfSame(path string, expected os.FileInfo) error {
	info, err := os.Lstat(path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil
		}
		return fmt.Errorf("inspect unix socket path %s: %w", path, err)
	}
	if info.Mode()&os.ModeSocket == 0 {
		return fmt.Errorf("refusing to remove non-socket path %s (mode %s)", path, info.Mode())
	}
	if expected == nil || !os.SameFile(expected, info) {
		return fmt.Errorf("refusing to remove unix socket %s because its identity changed", path)
	}
	if err := os.Remove(path); err != nil && !errors.Is(err, os.ErrNotExist) {
		return fmt.Errorf("remove unix socket %s: %w", path, err)
	}
	return nil
}

// Start runs the HTTP server loop.
func (s *Server) Start() error {
	err := s.httpServer.Serve(s.listener)
	if errors.Is(err, http.ErrServerClosed) || errors.Is(err, net.ErrClosed) {
		return nil
	}
	return err
}

// Stop gracefully shuts down daemon server and releases all resources owned by
// the Server, including the state Store opened by NewServer.
func (s *Server) Stop(ctx context.Context) error {
	shutdownErr := s.httpServer.Shutdown(ctx)
	listenerErr := s.listener.Close()
	if errors.Is(listenerErr, net.ErrClosed) {
		listenerErr = nil
	}

	var socketErr error
	if s.network == "unix" {
		socketErr = removeUnixSocketIfSame(s.addr, s.socketInfo)
	}
	storeErr := s.store.Close()
	return errors.Join(shutdownErr, listenerErr, socketErr, storeErr)
}

func (s *Server) handleSystemInfo(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"version":    "minictl/1.2.0",
		"go_version": "go1.21+",
		"os":         "linux",
		"time":       time.Now().Format(time.RFC3339),
	})
}

func (s *Server) handleListContainers(w http.ResponseWriter, r *http.Request) {
	ctrs, err := s.store.List()
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, ctrs)
}

func (s *Server) handleContainerRoute(w http.ResponseWriter, r *http.Request) {
	path := strings.TrimPrefix(r.URL.Path, "/v1/containers/")
	parts := strings.Split(path, "/")
	if len(parts) == 0 || parts[0] == "" {
		http.Error(w, "missing container id", http.StatusBadRequest)
		return
	}

	id := parts[0]

	if len(parts) == 1 && r.Method == http.MethodGet {
		c, err := s.store.Resolve(id)
		if err != nil {
			writeJSON(w, http.StatusNotFound, map[string]string{"error": err.Error()})
			return
		}
		writeJSON(w, http.StatusOK, c)
		return
	}

	if len(parts) == 1 && r.Method == http.MethodDelete {
		s.handleDeleteContainer(w, id)
		return
	}

	if len(parts) == 2 && parts[1] == "stop" && r.Method == http.MethodPost {
		s.handleStopContainer(w, r, id)
		return
	}

	http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
}

func (s *Server) handleListImages(w http.ResponseWriter, r *http.Request) {
	imgs, err := s.store.ListImages()
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}

	for _, img := range imgs {
		if img == nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "image list contains nil metadata"})
			return
		}
		if img.Size != 0 || img.RootFS == "" {
			continue
		}
		sz, err := imagestore.CalculateDirSize(img.RootFS)
		if err != nil {
			selector := strings.TrimSpace(img.Name)
			if selector == "" {
				selector = strings.TrimSpace(img.ID)
			}
			writeJSON(w, http.StatusInternalServerError, map[string]string{
				"error": fmt.Sprintf("calculate rootfs size for image %q at %q: %v", selector, img.RootFS, err),
			})
			return
		}
		img.Size = sz
	}
	writeJSON(w, http.StatusOK, imgs)
}

func (s *Server) handleStats(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		w.Header().Set("Allow", http.MethodGet)
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}

	interval := 200 * time.Millisecond
	if raw := r.URL.Query().Get("interval"); raw != "" {
		parsed, err := time.ParseDuration(raw)
		if err != nil {
			writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid interval: " + err.Error()})
			return
		}
		interval = parsed
	}
	if interval < 10*time.Millisecond || interval > 5*time.Second {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "interval must be between 10ms and 5s"})
		return
	}

	values, err := runtimestats.CollectStatsSampled(s.store, interval)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}
	if values == nil {
		values = []runtimestats.ContainerStat{}
	}
	writeJSON(w, http.StatusOK, values)
}

func (s *Server) handleMetrics(w http.ResponseWriter, r *http.Request) {
	content, err := metrics.GeneratePrometheusMetrics(s.store)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	w.Header().Set("Content-Type", "text/plain; version=0.0.4")
	w.WriteHeader(http.StatusOK)
	_, _ = w.Write([]byte(content))
}

func writeJSON(w http.ResponseWriter, code int, v interface{}) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(code)
	_ = json.NewEncoder(w).Encode(v)
}
