package volume

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"time"

	"minicontainer/internal/imagestore"
	"minicontainer/internal/state"
)

var validVolumeNameRegex = regexp.MustCompile(`^[a-zA-Z0-9][a-zA-Z0-9_.-]*$`)

// Volume holds persistent volume metadata.
type Volume struct {
	Name      string    `json:"name"`
	MountPath string    `json:"mount_path"`
	CreatedAt time.Time `json:"created_at"`
	Size      int64     `json:"size"`
}

func DefaultVolumeDir() string {
	return filepath.Join(state.DefaultDir(), "volumes")
}

// ValidateVolumeName checks that the volume name adheres to alphanumeric naming
// conventions and does not escape the default volumes storage root.
func ValidateVolumeName(name string) error {
	if name == "" {
		return fmt.Errorf("volume name cannot be empty")
	}
	if name == "." || name == ".." {
		return fmt.Errorf("invalid volume name %q: relative path components not allowed", name)
	}
	if strings.ContainsAny(name, "/\\:") {
		return fmt.Errorf("invalid volume name %q: path separators not allowed", name)
	}
	if !validVolumeNameRegex.MatchString(name) {
		return fmt.Errorf("invalid volume name %q: must start with alphanumeric character and contain only [a-zA-Z0-9_.-]", name)
	}

	volDir := filepath.Clean(filepath.Join(DefaultVolumeDir(), name))
	parentDir := filepath.Clean(DefaultVolumeDir())
	rel, err := filepath.Rel(parentDir, volDir)
	if err != nil || rel == "." || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return fmt.Errorf("invalid volume name %q: path escapes volume directory", name)
	}
	return nil
}

func ensureRealDir(path, label string, create bool, mode os.FileMode) error {
	if create {
		if err := os.MkdirAll(path, mode); err != nil {
			return fmt.Errorf("create %s: %w", label, err)
		}
	}
	info, err := os.Lstat(path)
	if err != nil {
		return fmt.Errorf("inspect %s: %w", label, err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return fmt.Errorf("%s is not a real directory", label)
	}
	if err := os.Chmod(path, mode); err != nil {
		return fmt.Errorf("secure %s permissions: %w", label, err)
	}
	return nil
}

func volumeRoot(create bool) (string, error) {
	base := state.DefaultDir()
	if err := ensureRealDir(base, "state directory", create, 0o700); err != nil {
		return "", err
	}
	root := DefaultVolumeDir()
	if err := ensureRealDir(root, "volume storage directory", create, 0o700); err != nil {
		return "", err
	}
	return root, nil
}

func ensureVolumeLayout(root, name string, create bool) (volDir, dataPath string, err error) {
	volDir = filepath.Join(root, name)
	if create {
		if mkdirErr := os.Mkdir(volDir, 0o700); mkdirErr != nil && !os.IsExist(mkdirErr) {
			return "", "", fmt.Errorf("create volume directory: %w", mkdirErr)
		}
	}
	if err := ensureRealDir(volDir, fmt.Sprintf("volume %q directory", name), false, 0o700); err != nil {
		return "", "", err
	}

	dataPath = filepath.Join(volDir, "_data")
	if create {
		if mkdirErr := os.Mkdir(dataPath, 0o755); mkdirErr != nil && !os.IsExist(mkdirErr) {
			return "", "", fmt.Errorf("create volume data directory: %w", mkdirErr)
		}
	}
	if err := ensureRealDir(dataPath, fmt.Sprintf("volume %q data directory", name), false, 0o755); err != nil {
		return "", "", err
	}
	return volDir, dataPath, nil
}

func writeVolumeMetadata(volDir string, vol *Volume) error {
	data, err := json.MarshalIndent(vol, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal volume metadata: %w", err)
	}
	tmp, err := os.CreateTemp(volDir, ".volume-*.tmp")
	if err != nil {
		return fmt.Errorf("create volume metadata temp file: %w", err)
	}
	tmpName := tmp.Name()
	closed := false
	defer func() {
		if !closed {
			_ = tmp.Close()
		}
		_ = os.Remove(tmpName)
	}()
	if err := tmp.Chmod(0o600); err != nil {
		return fmt.Errorf("secure volume metadata temp file: %w", err)
	}
	if _, err := tmp.Write(data); err != nil {
		return fmt.Errorf("write volume metadata: %w", err)
	}
	if err := tmp.Sync(); err != nil {
		return fmt.Errorf("sync volume metadata: %w", err)
	}
	if err := tmp.Close(); err != nil {
		return fmt.Errorf("close volume metadata: %w", err)
	}
	closed = true

	metaPath := filepath.Join(volDir, "volume.json")
	if info, err := os.Lstat(metaPath); err == nil {
		if info.Mode()&os.ModeSymlink != 0 || !info.Mode().IsRegular() {
			return fmt.Errorf("volume metadata is not a regular file")
		}
	} else if !os.IsNotExist(err) {
		return fmt.Errorf("inspect volume metadata: %w", err)
	}
	if err := os.Rename(tmpName, metaPath); err != nil {
		return fmt.Errorf("replace volume metadata: %w", err)
	}
	return nil
}

func readVolume(root, name string) (*Volume, error) {
	volDir, dataPath, err := ensureVolumeLayout(root, name, false)
	if err != nil {
		return nil, err
	}
	metaPath := filepath.Join(volDir, "volume.json")
	info, err := os.Lstat(metaPath)
	if err != nil {
		return nil, fmt.Errorf("inspect volume metadata: %w", err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.Mode().IsRegular() {
		return nil, fmt.Errorf("volume metadata is not a regular file")
	}
	data, err := os.ReadFile(metaPath)
	if err != nil {
		return nil, fmt.Errorf("read volume metadata: %w", err)
	}
	var vol Volume
	if err := json.Unmarshal(data, &vol); err != nil {
		return nil, fmt.Errorf("decode volume metadata: %w", err)
	}
	if vol.Name != name {
		return nil, fmt.Errorf("volume metadata name %q does not match directory %q", vol.Name, name)
	}
	if filepath.Clean(vol.MountPath) != filepath.Clean(dataPath) {
		return nil, fmt.Errorf("volume %q metadata mount path %q does not match managed data path %q", name, vol.MountPath, dataPath)
	}
	sz, err := imagestore.CalculateDirSize(dataPath)
	if err != nil {
		return nil, fmt.Errorf("calculate volume size: %w", err)
	}
	vol.Size = sz
	return &vol, nil
}

// CreateVolume creates a new named persistent volume.
func CreateVolume(name string) (*Volume, error) {
	if err := ValidateVolumeName(name); err != nil {
		return nil, err
	}
	return createVolume(name, time.Now())
}

// GetVolume retrieves volume details by name.
func GetVolume(name string) (*Volume, error) {
	if err := ValidateVolumeName(name); err != nil {
		return nil, err
	}
	root, err := volumeRoot(false)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil, fmt.Errorf("volume %q not found", name)
		}
		return nil, err
	}
	vol, err := readVolume(root, name)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil, fmt.Errorf("volume %q not found", name)
		}
		return nil, err
	}
	return vol, nil
}

// ListVolumes lists all registered volumes. Entries with invalid volume names
// are outside the managed namespace and remain ignored. A valid-name entry is
// managed state, so structural or metadata failures must be reported rather
// than silently making a corrupt volume disappear from inventory.
func ListVolumes() ([]*Volume, error) {
	root, err := volumeRoot(false)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return []*Volume{}, nil
		}
		return nil, err
	}
	entries, err := os.ReadDir(root)
	if err != nil {
		return nil, err
	}
	var out []*Volume
	for _, entry := range entries {
		name := entry.Name()
		if err := ValidateVolumeName(name); err != nil {
			continue
		}
		if !entry.IsDir() {
			return nil, fmt.Errorf("managed volume entry %q is not a directory", name)
		}
		vol, err := readVolume(root, name)
		if err != nil {
			return nil, fmt.Errorf("read managed volume %q: %w", name, err)
		}
		out = append(out, vol)
	}
	return out, nil
}

// RemoveVolume deletes a named volume.
func RemoveVolume(name string) error {
	if err := ValidateVolumeName(name); err != nil {
		return err
	}
	root, err := volumeRoot(false)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return fmt.Errorf("volume %q not found", name)
		}
		return err
	}
	if _, _, err := ensureVolumeLayout(root, name, false); err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return fmt.Errorf("volume %q not found", name)
		}
		return err
	}
	// Require valid metadata before recursively deleting any managed directory.
	if _, err := readVolume(root, name); err != nil {
		return fmt.Errorf("validate volume before removal: %w", err)
	}
	if err := removeVolumeDir(root, name); err != nil {
		return fmt.Errorf("remove volume %q: %w", name, err)
	}
	return nil
}

// PruneVolumes removes every valid-name entry in the managed volume root and
// reports every failed removal instead of silently filtering corrupt state.
func PruneVolumes() (int, error) {
	root, err := volumeRoot(false)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return 0, nil
		}
		return 0, err
	}
	entries, err := os.ReadDir(root)
	if err != nil {
		return 0, fmt.Errorf("read volume storage directory for prune: %w", err)
	}
	count := 0
	var removeErrs []error
	for _, entry := range entries {
		name := entry.Name()
		if err := ValidateVolumeName(name); err != nil {
			continue
		}
		if err := RemoveVolume(name); err != nil {
			removeErrs = append(removeErrs, fmt.Errorf("remove volume %q during prune: %w", name, err))
			continue
		}
		count++
	}
	return count, errors.Join(removeErrs...)
}

// ResolveVolumePath returns the host directory path for a volume name or host path.
func ResolveVolumePath(spec string) string {
	if err := ValidateVolumeName(spec); err == nil {
		vol, err := GetVolume(spec)
		if err == nil && vol.MountPath != "" {
			return vol.MountPath
		}
	}
	return spec
}
