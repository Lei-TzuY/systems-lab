package plugin

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"regexp"

	"minicontainer/internal/state"
)

type PluginType string

const (
	PluginTypeVolume  PluginType = "volume"
	PluginTypeNetwork PluginType = "network"
	PluginTypeLog     PluginType = "log"
)

var validPluginName = regexp.MustCompile(`^[A-Za-z0-9][A-Za-z0-9_.-]*$`)

type Plugin struct {
	Name        string     `json:"name"`
	Version     string     `json:"version"`
	Type        PluginType `json:"type"`
	Executable  string     `json:"executable"`
	Description string     `json:"description,omitempty"`
	Enabled     bool       `json:"enabled"`
}

func PluginsDir() string {
	return filepath.Join(state.DefaultDir(), "plugins")
}

func validatePluginName(name string) error {
	if name == "" || name == "." || name == ".." || !validPluginName.MatchString(name) {
		return fmt.Errorf("invalid plugin name %q", name)
	}
	return nil
}

func ensurePluginStateBase() error {
	base := state.DefaultDir()
	if err := os.MkdirAll(base, 0o700); err != nil {
		return fmt.Errorf("create state directory: %w", err)
	}
	info, err := os.Lstat(base)
	if err != nil {
		return fmt.Errorf("inspect state directory: %w", err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return fmt.Errorf("state directory is not a real directory")
	}
	if err := os.Chmod(base, 0o700); err != nil {
		return fmt.Errorf("secure state directory permissions: %w", err)
	}
	return nil
}

func ensurePluginsDir() (string, error) {
	if err := ensurePluginStateBase(); err != nil {
		return "", err
	}
	dir := PluginsDir()
	if err := os.MkdirAll(dir, 0o700); err != nil {
		return "", fmt.Errorf("create plugins directory: %w", err)
	}
	info, err := os.Lstat(dir)
	if err != nil {
		return "", fmt.Errorf("inspect plugins directory: %w", err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return "", fmt.Errorf("plugins directory is not a real directory")
	}
	if err := os.Chmod(dir, 0o700); err != nil {
		return "", fmt.Errorf("secure plugins directory: %w", err)
	}
	return dir, nil
}

func ensurePluginDir(root, name string) (string, error) {
	pDir := filepath.Join(root, name)
	if err := os.Mkdir(pDir, 0o700); err != nil && !os.IsExist(err) {
		return "", fmt.Errorf("create plugin directory: %w", err)
	}
	info, err := os.Lstat(pDir)
	if err != nil {
		return "", fmt.Errorf("inspect plugin directory: %w", err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return "", fmt.Errorf("plugin directory %q is not a real directory", name)
	}
	if err := os.Chmod(pDir, 0o700); err != nil {
		return "", fmt.Errorf("secure plugin directory: %w", err)
	}
	return pDir, nil
}

func writePluginManifest(dir string, p Plugin) error {
	data, err := json.MarshalIndent(p, "", "  ")
	if err != nil {
		return err
	}

	tmp, err := os.CreateTemp(dir, ".plugin-*.tmp")
	if err != nil {
		return fmt.Errorf("create plugin manifest temp file: %w", err)
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
		return fmt.Errorf("secure plugin manifest temp file: %w", err)
	}
	if _, err := tmp.Write(data); err != nil {
		return fmt.Errorf("write plugin manifest: %w", err)
	}
	if err := tmp.Sync(); err != nil {
		return fmt.Errorf("sync plugin manifest: %w", err)
	}
	if err := tmp.Close(); err != nil {
		return fmt.Errorf("close plugin manifest: %w", err)
	}
	closed = true

	manifest := filepath.Join(dir, "plugin.json")
	if info, err := os.Lstat(manifest); err == nil {
		if info.Mode()&os.ModeSymlink != 0 || !info.Mode().IsRegular() {
			return fmt.Errorf("plugin manifest is not a regular file")
		}
	} else if !os.IsNotExist(err) {
		return fmt.Errorf("inspect plugin manifest: %w", err)
	}
	if err := os.Rename(tmpName, manifest); err != nil {
		return fmt.Errorf("replace plugin manifest: %w", err)
	}
	return nil
}

// ListPlugins reads all plugin manifests from ~/.minicontainer/plugins/.
func ListPlugins() ([]Plugin, error) {
	dir, err := ensurePluginsDir()
	if err != nil {
		return nil, err
	}

	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil, err
	}

	var plugins []Plugin
	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}
		manifestPath := filepath.Join(dir, entry.Name(), "plugin.json")
		info, err := os.Lstat(manifestPath)
		if err != nil || info.Mode()&os.ModeSymlink != 0 || !info.Mode().IsRegular() {
			continue
		}
		data, err := os.ReadFile(manifestPath)
		if err != nil {
			continue
		}
		var p Plugin
		if err := json.Unmarshal(data, &p); err != nil {
			continue
		}
		if p.Name != entry.Name() || validatePluginName(p.Name) != nil {
			continue
		}
		plugins = append(plugins, p)
	}
	return plugins, nil
}

// InstallPlugin creates or updates a plugin manifest under ~/.minicontainer/plugins/<name>/.
func InstallPlugin(name, version string, pType PluginType, execPath string, desc string) error {
	if err := validatePluginName(name); err != nil {
		return err
	}
	root, err := ensurePluginsDir()
	if err != nil {
		return err
	}
	pDir, err := ensurePluginDir(root, name)
	if err != nil {
		return err
	}

	p := Plugin{Name: name, Version: version, Type: pType, Executable: execPath, Description: desc, Enabled: true}
	return writePluginManifest(pDir, p)
}

// RemovePlugin deletes a plugin directory.
func RemovePlugin(name string) error {
	if err := validatePluginName(name); err != nil {
		return err
	}
	root, err := ensurePluginsDir()
	if err != nil {
		return err
	}
	pDir := filepath.Join(root, name)
	info, err := os.Lstat(pDir)
	if err != nil {
		if os.IsNotExist(err) {
			return fmt.Errorf("plugin %q not found", name)
		}
		return fmt.Errorf("inspect plugin directory: %w", err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return fmt.Errorf("plugin directory %q is not a real directory", name)
	}
	return os.RemoveAll(pDir)
}
