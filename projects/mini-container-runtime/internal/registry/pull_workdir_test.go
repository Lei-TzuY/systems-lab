package registry

import "testing"

func TestParseImageRuntimeConfigWorkingDir(t *testing.T) {
	cfg, err := parseImageRuntimeConfig([]byte(`{"config":{"WorkingDir":"/srv/app/../work"}}`))
	if err != nil {
		t.Fatalf("parseImageRuntimeConfig() error = %v", err)
	}
	if cfg.WorkingDir != "/srv/work" {
		t.Fatalf("WorkingDir = %q, want /srv/work", cfg.WorkingDir)
	}
}

func TestParseImageRuntimeConfigRejectsRelativeWorkingDir(t *testing.T) {
	if _, err := parseImageRuntimeConfig([]byte(`{"config":{"WorkingDir":"relative/path"}}`)); err == nil {
		t.Fatal("relative WorkingDir unexpectedly accepted")
	}
}
