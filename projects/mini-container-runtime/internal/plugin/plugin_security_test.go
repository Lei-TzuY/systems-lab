package plugin

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestPluginTraversalCannotWriteOrDeleteOutsideRoot(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	victim := filepath.Join(home, ".minicontainer", "victim")
	if err := os.MkdirAll(victim, 0o700); err != nil { t.Fatal(err) }
	sentinel := filepath.Join(victim, "sentinel")
	if err := os.WriteFile(sentinel, []byte("keep"), 0o600); err != nil { t.Fatal(err) }
	if err := InstallPlugin("../victim", "1", PluginTypeLog, "/bin/true", ""); err == nil || !strings.Contains(err.Error(), "invalid plugin name") { t.Fatalf("InstallPlugin traversal error=%v", err) }
	if err := RemovePlugin("../victim"); err == nil || !strings.Contains(err.Error(), "invalid plugin name") { t.Fatalf("RemovePlugin traversal error=%v", err) }
	if data, err := os.ReadFile(sentinel); err != nil || string(data) != "keep" { t.Fatalf("outside victim changed: data=%q err=%v", data, err) }
	if _, err := os.Stat(filepath.Join(victim, "plugin.json")); !os.IsNotExist(err) { t.Fatalf("traversal install wrote outside plugin root: %v", err) }
}

func TestInstallPluginRejectsSymlinkedPluginDirectory(t *testing.T) {
	home := t.TempDir(); t.Setenv("HOME", home)
	root := PluginsDir(); if err := os.MkdirAll(root, 0o700); err != nil { t.Fatal(err) }
	outside := t.TempDir(); sentinel := filepath.Join(outside, "sentinel")
	if err := os.WriteFile(sentinel, []byte("keep"), 0o600); err != nil { t.Fatal(err) }
	if err := os.Symlink(outside, filepath.Join(root, "driver")); err != nil { t.Fatal(err) }
	if err := InstallPlugin("driver", "1", PluginTypeVolume, "/bin/true", ""); err == nil || !strings.Contains(err.Error(), "not a real directory") { t.Fatalf("InstallPlugin symlinked dir error=%v", err) }
	if data, err := os.ReadFile(sentinel); err != nil || string(data) != "keep" { t.Fatalf("outside target changed: data=%q err=%v", data, err) }
	if _, err := os.Stat(filepath.Join(outside, "plugin.json")); !os.IsNotExist(err) { t.Fatalf("symlinked plugin dir received manifest: %v", err) }
}

func TestInstallPluginRejectsSymlinkedManifest(t *testing.T) {
	home := t.TempDir(); t.Setenv("HOME", home)
	pDir := filepath.Join(PluginsDir(), "driver"); if err := os.MkdirAll(pDir, 0o700); err != nil { t.Fatal(err) }
	outside := filepath.Join(t.TempDir(), "outside.json"); const sentinel = "do-not-overwrite"
	if err := os.WriteFile(outside, []byte(sentinel), 0o600); err != nil { t.Fatal(err) }
	if err := os.Symlink(outside, filepath.Join(pDir, "plugin.json")); err != nil { t.Fatal(err) }
	if err := InstallPlugin("driver", "2", PluginTypeNetwork, "/bin/true", ""); err == nil || !strings.Contains(err.Error(), "manifest is not a regular file") { t.Fatalf("InstallPlugin symlinked manifest error=%v", err) }
	if data, err := os.ReadFile(outside); err != nil || string(data) != sentinel { t.Fatalf("symlink manifest target changed: data=%q err=%v", data, err) }
}

func TestPluginRootSymlinkRejected(t *testing.T) {
	home := t.TempDir(); t.Setenv("HOME", home)
	stateDir := filepath.Join(home, ".minicontainer"); if err := os.MkdirAll(stateDir, 0o700); err != nil { t.Fatal(err) }
	outside := t.TempDir(); if err := os.Symlink(outside, filepath.Join(stateDir, "plugins")); err != nil { t.Fatal(err) }
	if err := InstallPlugin("driver", "1", PluginTypeLog, "/bin/true", ""); err == nil || !strings.Contains(err.Error(), "plugins directory is not a real directory") { t.Fatalf("InstallPlugin symlinked root error=%v", err) }
	if _, err := ListPlugins(); err == nil || !strings.Contains(err.Error(), "plugins directory is not a real directory") { t.Fatalf("ListPlugins symlinked root error=%v", err) }
	if _, err := os.Stat(filepath.Join(outside, "driver", "plugin.json")); !os.IsNotExist(err) { t.Fatalf("symlinked plugin root received data: %v", err) }
}

func TestListPluginsDoesNotReadSymlinkManifest(t *testing.T) {
	home := t.TempDir(); t.Setenv("HOME", home)
	pDir := filepath.Join(PluginsDir(), "driver"); if err := os.MkdirAll(pDir, 0o700); err != nil { t.Fatal(err) }
	outside := filepath.Join(t.TempDir(), "manifest.json")
	if err := os.WriteFile(outside, []byte(`{"name":"driver"}`), 0o600); err != nil { t.Fatal(err) }
	if err := os.Symlink(outside, filepath.Join(pDir, "plugin.json")); err != nil { t.Fatal(err) }
	plugins, err := ListPlugins(); if err != nil { t.Fatalf("ListPlugins: %v", err) }
	if len(plugins) != 0 { t.Fatalf("ListPlugins followed symlink manifest: %+v", plugins) }
}

func TestPluginStorageUsesPrivateModesAndSupportsUpdate(t *testing.T) {
	home := t.TempDir(); t.Setenv("HOME", home)
	if err := InstallPlugin("driver0", "1", PluginTypeLog, "/bin/true", "first"); err != nil { t.Fatal(err) }
	if err := InstallPlugin("driver0", "2", PluginTypeLog, "/bin/true", "second"); err != nil { t.Fatalf("update plugin: %v", err) }
	rootInfo, err := os.Stat(PluginsDir()); if err != nil { t.Fatal(err) }; if rootInfo.Mode().Perm() != 0o700 { t.Fatalf("plugins dir mode=%#o", rootInfo.Mode().Perm()) }
	pDir := filepath.Join(PluginsDir(), "driver0"); pInfo, err := os.Stat(pDir); if err != nil { t.Fatal(err) }; if pInfo.Mode().Perm() != 0o700 { t.Fatalf("plugin dir mode=%#o", pInfo.Mode().Perm()) }
	mInfo, err := os.Stat(filepath.Join(pDir, "plugin.json")); if err != nil { t.Fatal(err) }; if mInfo.Mode().Perm() != 0o600 { t.Fatalf("manifest mode=%#o", mInfo.Mode().Perm()) }
	plugins, err := ListPlugins(); if err != nil || len(plugins) != 1 || plugins[0].Version != "2" { t.Fatalf("updated plugins=%+v err=%v", plugins, err) }
}
