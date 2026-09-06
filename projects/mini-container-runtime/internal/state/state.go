// internal/state/state.go
//
// Container State Store
// ─────────────────────
// A real container runtime (Docker, containerd) stores state about running
// and stopped containers in a database (BoltDB in Docker, leveldb in
// containerd).  We use a simpler approach: one JSON file per container
// under a state directory, analogous to runc's state.json.
//
// State directory layout:
//
//   ~/.minicontainer/
//   ├── containers/
//   │   ├── <id>.json      ← Container record (see Container struct)
//   │   └── <id>.json
//   └── images/
//       └── <name>.json    ← Image metadata (see Image struct)

package state

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"
)

// DefaultDir returns ~/.minicontainer.
func DefaultDir() string {
	home, err := os.UserHomeDir()
	if err != nil {
		home = "/tmp"
	}
	return filepath.Join(home, ".minicontainer")
}

// Status represents the lifecycle state of a container.
type Status string

const (
	StatusCreated Status = "created"
	StatusRunning Status = "running"
	StatusStopped Status = "stopped"
)

const (
	maxContainerIDBytes  = 240
	maxImageIdentityBytes = 4096
)

// Container holds persisted metadata for one container.
type Container struct {
	// ID is a short random hex string used as a human-readable handle.
	ID string `json:"id"`

	// Revision is an optimistic concurrency token. Every successful container
	// state write increments it; Save rejects snapshots whose revision no longer
	// matches disk, preventing stale read-modify-write callers from overwriting
	// newer runtime lifecycle state.
	Revision uint64 `json:"revision,omitempty"`

	// PID is the host PID of the container's init process.
	PID int `json:"pid"`

	// PIDStartTime is Linux /proc/<pid>/stat field 22 (clock ticks since boot).
	// Pairing it with PID lets lifecycle operations detect PID reuse before
	// signaling what may now be an unrelated host process.
	PIDStartTime uint64 `json:"pid_start_time,omitempty"`

	// Status is the current lifecycle state of a container.
	Status Status `json:"status"`

	// Health holds container health status ("healthy", "unhealthy", "starting", or "").
	Health string `json:"health,omitempty"`

	// RootFS is the rootfs directory path given at container creation.
	RootFS string `json:"rootfs"`

	// Command is the argv of the process running inside the container.
	Command []string `json:"command"`

	// Hostname is the UTS hostname set inside the container.
	Hostname string `json:"hostname"`

	// CreatedAt is the wall-clock time the container was created.
	CreatedAt time.Time `json:"created_at"`

	// StartedAt is when the init process was started.
	StartedAt *time.Time `json:"started_at,omitempty"`

	// FinishedAt is when the init process exited.
	FinishedAt *time.Time `json:"finished_at,omitempty"`

	// ExitCode is the exit status code of the container process (-1 if killed).
	ExitCode int `json:"exit_code"`

	// Env is environment variables set inside container.
	Env []string `json:"env,omitempty"`
}

// Image holds metadata for a registered rootfs image.
type Image struct {
	ID           string    `json:"id,omitempty"`
	Repository   string    `json:"repository,omitempty"`
	Tag          string    `json:"tag,omitempty"`
	Name         string    `json:"name"`
	RootFS       string    `json:"rootfs"`
	Size         int64     `json:"size,omitempty"`
	LoadedAt     time.Time `json:"loaded_at"`
	WorkDir      string    `json:"work_dir,omitempty"`
	Env          []string  `json:"env,omitempty"`
	Cmd          []string  `json:"cmd,omitempty"`
	ExposedPorts []string  `json:"exposed_ports,omitempty"`
}

// Store handles process-local serialization plus a Linux cross-process lock for
// mutations under one state directory. On Linux, the state, container, and
// image directories are pinned for the Store lifetime so later pathname
// replacement cannot redirect an already-open Store to another filesystem tree.
type Store struct {
	mu          sync.Mutex
	dir         string
	ctrDir      string
	imgDir      string
	lockFile    *os.File
	storagePins []*os.File
}

func Open(dir string) (*Store, error) {
	if err := ensurePrivateStateDir(dir, "state"); err != nil {
		return nil, err
	}
	ctrDir := filepath.Join(dir, "containers")
	imgDir := filepath.Join(dir, "images")
	if err := ensurePrivateStateDir(ctrDir, "container state"); err != nil {
		return nil, err
	}
	if err := ensurePrivateStateDir(imgDir, "image state"); err != nil {
		return nil, err
	}

	pinned, err := pinStateStorage(dir)
	if err != nil {
		return nil, err
	}
	lockFile, err := openStateLock(filepath.Join(pinned.rootDir, ".state.lock"))
	if err != nil {
		closePinnedStateStorage(pinned)
		return nil, err
	}

	return &Store{
		dir:         dir,
		ctrDir:      pinned.ctrDir,
		imgDir:      pinned.imgDir,
		lockFile:    lockFile,
		storagePins: pinned.files,
	}, nil
}

// Dir returns the durable configured state-root pathname. Call StoragePath for
// filesystem mutations that must fail if that pathname no longer references
// the directory generation pinned by Open.
func (s *Store) Dir() string {
	return s.dir
}

func validateID(id string) error {
	if id == "" {
		return fmt.Errorf("id cannot be empty")
	}
	if len(id) > maxContainerIDBytes {
		return fmt.Errorf("invalid id: exceeds %d bytes", maxContainerIDBytes)
	}
	if id == "." || id == ".." || strings.ContainsAny(id, "/\\:\x00") {
		return fmt.Errorf("invalid id %q: path separators and relative components not allowed", id)
	}
	return nil
}

func validateImageSelector(nameOrID string) error {
	if strings.TrimSpace(nameOrID) == "" {
		return fmt.Errorf("image name or ID cannot be empty")
	}
	if len(nameOrID) > maxImageIdentityBytes {
		return fmt.Errorf("image name or ID exceeds %d bytes", maxImageIdentityBytes)
	}
	return nil
}

func atomicWriteFile(dir, target string, data []byte) error {
	tmpFile, err := os.CreateTemp(dir, ".tmp-*")
	if err != nil {
		return fmt.Errorf("create state tmp file: %w", err)
	}
	tmpName := tmpFile.Name()
	closed := false
	defer func() {
		if !closed {
			_ = tmpFile.Close()
		}
		_ = os.Remove(tmpName)
	}()

	if err := tmpFile.Chmod(0o600); err != nil {
		return fmt.Errorf("secure state tmp file permissions: %w", err)
	}
	if _, err := tmpFile.Write(data); err != nil {
		return fmt.Errorf("write state tmp file: %w", err)
	}
	if err := tmpFile.Sync(); err != nil {
		return fmt.Errorf("sync state tmp file: %w", err)
	}
	if err := tmpFile.Close(); err != nil {
		return fmt.Errorf("close state tmp file: %w", err)
	}
	closed = true

	var renameErr error
	for attempts := 0; attempts < 10; attempts++ {
		renameErr = os.Rename(tmpName, target)
		if renameErr == nil {
			return syncStateDirectory(dir, "state")
		}
		time.Sleep(time.Duration(attempts+1) * 2 * time.Millisecond)
	}

	return fmt.Errorf("atomic rename state file: %w", renameErr)
}

func (s *Store) Save(c *Container) error {
	if c == nil {
		return fmt.Errorf("container state is nil")
	}
	if err := validateID(c.ID); err != nil {
		return err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	return s.saveContainerCASUnlocked(c)
}

func (s *Store) Get(id string) (*Container, error) {
	if err := validateID(id); err != nil {
		return nil, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	return s.getUnlocked(id)
}

func (s *Store) Load(id string) (*Container, error) {
	return s.Get(id)
}

func (s *Store) getUnlocked(id string) (*Container, error) {
	if s.lockFile == nil {
		return nil, ErrStoreClosed
	}
	file := filepath.Join(s.ctrDir, id+".json")
	data, err := readRegularStateFile(file, "container state")
	if err != nil {
		if os.IsNotExist(err) {
			return nil, fmt.Errorf("container %q not found: %w", id, err)
		}
		return nil, fmt.Errorf("read container state: %w", err)
	}

	var c Container
	if err := unmarshalContainerStateForID(data, id, &c); err != nil {
		return nil, fmt.Errorf("unmarshal container state: %w", err)
	}
	return &c, nil
}

func (s *Store) Resolve(idOrPrefix string) (*Container, error) {
	if err := validateID(idOrPrefix); err != nil {
		return nil, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if s.lockFile == nil {
		return nil, ErrStoreClosed
	}

	c, exactErr := s.getUnlocked(idOrPrefix)
	if exactErr == nil {
		return c, nil
	}
	if !errors.Is(exactErr, os.ErrNotExist) {
		return nil, fmt.Errorf("resolve exact container %q: %w", idOrPrefix, exactErr)
	}

	entries, err := os.ReadDir(s.ctrDir)
	if err != nil {
		return nil, fmt.Errorf("read container state dir: %w", err)
	}

	var matches []string
	for _, entry := range entries {
		if entry.IsDir() || filepath.Ext(entry.Name()) != ".json" {
			continue
		}
		name := entry.Name()
		id := name[:len(name)-len(".json")]
		if len(id) >= len(idOrPrefix) && id[:len(idOrPrefix)] == idOrPrefix {
			matches = append(matches, id)
		}
	}

	if len(matches) == 0 {
		return nil, fmt.Errorf("no container matched prefix %q", idOrPrefix)
	}
	if len(matches) > 1 {
		return nil, fmt.Errorf("ambiguous container prefix %q matched multiple IDs (%v)", idOrPrefix, matches)
	}

	return s.getUnlocked(matches[0])
}

func (s *Store) List() ([]*Container, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.lockFile == nil {
		return nil, ErrStoreClosed
	}

	entries, err := os.ReadDir(s.ctrDir)
	if err != nil {
		return nil, fmt.Errorf("read container state dir: %w", err)
	}

	var out []*Container
	for _, entry := range entries {
		if entry.IsDir() || filepath.Ext(entry.Name()) != ".json" {
			continue
		}
		id := entry.Name()[:len(entry.Name())-len(".json")]
		c, err := s.getUnlocked(id)
		if err != nil {
			return nil, fmt.Errorf("load container state %q: %w", id, err)
		}
		out = append(out, c)
	}
	return out, nil
}

func (s *Store) Delete(id string) error {
	if err := validateID(id); err != nil {
		return err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	ownership, ok, err := s.readCgroupOwnershipUnlocked(id)
	if err != nil {
		return fmt.Errorf("read pending cgroup ownership before deleting container %s: %w", id, err)
	}
	if ok {
		return fmt.Errorf(
			"container %s has pending cgroup cleanup for %s (%d/%d)",
			id,
			ownership.Name,
			ownership.PID,
			ownership.PIDStartTime,
		)
	}

	networkOwnership, ok, err := s.readNetworkOwnershipUnlocked(id)
	if err != nil {
		return fmt.Errorf("read pending network ownership before deleting container %s: %w", id, err)
	}
	if ok {
		return fmt.Errorf(
			"container %s has pending network cleanup for %s (%d/%d)",
			id,
			networkOwnership.Owner,
			networkOwnership.PID,
			networkOwnership.PIDStartTime,
		)
	}

	file := filepath.Join(s.ctrDir, id+".json")
	return removeStateFileDurable(s.ctrDir, file, "container state")
}

func (s *Store) SaveImage(img *Image) error {
	if _, err := imageStorageKey(img); err != nil {
		return err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	data, err := json.MarshalIndent(img, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal image: %w", err)
	}
	return s.saveImageMetadataUnlocked(img, data)
}

func (s *Store) GetImage(nameOrID string) (*Image, error) {
	if err := validateImageSelector(nameOrID); err != nil {
		return nil, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	images, err := s.listImagesUnlocked()
	if err != nil {
		return nil, err
	}
	return resolveImageForRead(images, nameOrID)
}

func (s *Store) DeleteImage(nameOrID string) (*Image, error) {
	if err := validateImageSelector(nameOrID); err != nil {
		return nil, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return nil, err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	images, err := s.listImagesUnlocked()
	if err != nil {
		return nil, err
	}
	img, err := resolveImageForDelete(images, nameOrID)
	if err != nil {
		return nil, err
	}
	if err := s.removeImageMetadataUnlocked(img); err != nil {
		return nil, err
	}
	return img, nil
}

// GetImageUnlocked is retained for compatibility with image-store callers, but
// now serializes with Close so it cannot race pinned-directory teardown.
func (s *Store) GetImageUnlocked(nameOrID string) (*Image, error) {
	if err := validateImageSelector(nameOrID); err != nil {
		return nil, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	images, err := s.listImagesUnlocked()
	if err != nil {
		return nil, err
	}
	return resolveImageForRead(images, nameOrID)
}

func (s *Store) ListImages() ([]*Image, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	return s.listImagesUnlocked()
}

func (s *Store) listImagesUnlocked() ([]*Image, error) {
	if s.lockFile == nil {
		return nil, ErrStoreClosed
	}
	entries, err := os.ReadDir(s.imgDir)
	if err != nil {
		return nil, fmt.Errorf("read image state dir: %w", err)
	}

	var out []*Image
	seen := make(map[string]seenImageMetadata)
	for _, entry := range entries {
		if entry.IsDir() || filepath.Ext(entry.Name()) != ".json" {
			continue
		}
		path := filepath.Join(s.imgDir, entry.Name())
		img, err := readImageMetadata(path)
		if err != nil {
			return nil, fmt.Errorf("read image state %q: %w", entry.Name(), err)
		}
		out, err = appendUniqueImageMetadata(out, seen, img, path)
		if err != nil {
			return nil, fmt.Errorf("image state %q: %w", entry.Name(), err)
		}
	}
	return out, nil
}

func sanitizeImageFilename(name string) string {
	r := strings.NewReplacer("/", "_", ":", "_", "\\", "_", "..", "_")
	cleaned := strings.Trim(r.Replace(name), " ._")
	if cleaned == "" {
		return "default"
	}
	return cleaned
}