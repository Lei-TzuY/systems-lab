package plugin

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestPluginStateBaseSymlinkCannotRedirectStorage(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	outside := t.TempDir()
	victim := filepath.Join(outside, "plugins", "driver")
	if err := os.MkdirAll(victim, 0o700); err != nil {
		t.Fatal(err)
	}
	sentinel := filepath.Join(victim, "sentinel")
	if err := os.WriteFile(sentinel, []byte("keep"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, filepath.Join(home, ".minicontainer")); err != nil {
		t.Fatal(err)
	}

	if err := InstallPlugin("newdriver", "1", PluginTypeLog, "/bin/true", ""); err == nil || !strings.Contains(err.Error(), "state directory is not a real directory") {
		t.Fatalf("InstallPlugin symlink state base error=%v", err)
	}
	if err := RemovePlugin("driver"); err == nil || !strings.Contains(err.Error(), "state directory is not a real directory") {
		t.Fatalf("RemovePlugin symlink state base error=%v", err)
	}
	if _, err := ListPlugins(); err == nil || !strings.Contains(err.Error(), "state directory is not a real directory") {
		t.Fatalf("ListPlugins symlink state base error=%v", err)
	}
	if data, err := os.ReadFile(sentinel); err != nil || string(data) != "keep" {
		t.Fatalf("outside plugin data changed: data=%q err=%v", data, err)
	}
	if _, err := os.Stat(filepath.Join(outside, "plugins", "newdriver", "plugin.json")); !os.IsNotExist(err) {
		t.Fatalf("InstallPlugin wrote through symlinked state base: %v", err)
	}
}
