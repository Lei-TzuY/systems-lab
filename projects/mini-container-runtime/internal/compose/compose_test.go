package compose

import (
	"os"
	"path/filepath"
	"testing"
)

func TestParseConfigFile(t *testing.T) {
	tmpDir := t.TempDir()
	jsonFile := filepath.Join(tmpDir, "minicontose.json")

	content := `{
		"version": "1.0",
		"services": {
			"web": {
				"image": "./rootfs-web",
				"command": ["/bin/nc", "-l", "-p", "80"],
				"hostname": "web-srv",
				"workdir": "/app",
				"overlay": true,
				"environment": {
					"PORT": "80",
					"ENV": "production"
				}
			}
		}
	}`

	if err := os.WriteFile(jsonFile, []byte(content), 0644); err != nil {
		t.Fatalf("write temp json: %v", err)
	}

	cfg, err := ParseConfigFile(jsonFile)
	if err != nil {
		t.Fatalf("ParseConfigFile error: %v", err)
	}

	if cfg.Version != "1.0" {
		t.Fatalf("Version = %q, want 1.0", cfg.Version)
	}
	web, ok := cfg.Services["web"]
	if !ok {
		t.Fatalf("web service not found")
	}

	cCfg := web.BuildContainerConfig("web")
	if cCfg.RootFS != "./rootfs-web" {
		t.Fatalf("RootFS = %q", cCfg.RootFS)
	}
	if cCfg.Hostname != "web-srv" {
		t.Fatalf("Hostname = %q", cCfg.Hostname)
	}
	if !cCfg.Overlay {
		t.Fatalf("Overlay = false, want true")
	}
	if len(cCfg.Env) != 2 {
		t.Fatalf("len(Env) = %d, want 2", len(cCfg.Env))
	}
}
