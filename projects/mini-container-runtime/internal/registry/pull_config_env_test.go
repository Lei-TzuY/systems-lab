package registry

import (
	"reflect"
	"strings"
	"testing"
)

func TestParseImageRuntimeConfigIncludesEnvironment(t *testing.T) {
	cfg, err := parseImageRuntimeConfig([]byte("{\"config\":{\"StopSignal\":\"SIGUSR1\",\"Env\":[\"PATH=/bin\",\"FOO=bar\"]}}"))
	if err != nil {
		t.Fatalf("parseImageRuntimeConfig() error = %v", err)
	}
	if cfg.StopSignal != "SIGUSR1" {
		t.Fatalf("StopSignal = %q, want SIGUSR1", cfg.StopSignal)
	}
	wantEnv := []string{"PATH=/bin", "FOO=bar"}
	if !reflect.DeepEqual(cfg.Env, wantEnv) {
		t.Fatalf("Env = %#v, want %#v", cfg.Env, wantEnv)
	}
}

func TestParseImageRuntimeConfigRejectsMalformedEnvironment(t *testing.T) {
	_, err := parseImageRuntimeConfig([]byte("{\"config\":{\"Env\":[\"BROKEN\"]}}"))
	if err == nil {
		t.Fatal("parseImageRuntimeConfig() returned nil error for malformed environment")
	}
	if !strings.Contains(err.Error(), "invalid image environment entry") {
		t.Fatalf("error = %q, want invalid image environment entry", err)
	}
}
