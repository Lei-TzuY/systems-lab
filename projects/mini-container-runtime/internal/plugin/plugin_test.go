package plugin

import (
	"testing"
)

func TestPluginManagement(t *testing.T) {
	tmpDir := t.TempDir()
	t.Setenv("HOME", tmpDir)

	err := InstallPlugin("my-vol-driver", "1.0.0", PluginTypeVolume, "/usr/bin/my-vol", "Custom volume driver")
	if err != nil {
		t.Fatalf("InstallPlugin error: %v", err)
	}

	plugins, err := ListPlugins()
	if err != nil || len(plugins) != 1 {
		t.Fatalf("ListPlugins count = %d, want 1, err: %v", len(plugins), err)
	}

	if plugins[0].Name != "my-vol-driver" {
		t.Fatalf("Plugin name = %s, want my-vol-driver", plugins[0].Name)
	}

	if err := RemovePlugin("my-vol-driver"); err != nil {
		t.Fatalf("RemovePlugin error: %v", err)
	}

	pluginsAfter, _ := ListPlugins()
	if len(pluginsAfter) != 0 {
		t.Fatalf("ListPlugins after remove count = %d, want 0", len(pluginsAfter))
	}
}
