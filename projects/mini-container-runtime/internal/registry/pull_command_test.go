package registry

import (
	"reflect"
	"strings"
	"testing"
)

func TestParseImageRuntimeConfigCommand(t *testing.T) {
	cfg, err := parseImageRuntimeConfig([]byte(`{"config":{"Entrypoint":["/bin/app"],"Cmd":["serve","--port=8080"]}}`))
	if err != nil {
		t.Fatalf("parseImageRuntimeConfig() error = %v", err)
	}
	if !reflect.DeepEqual(cfg.Entrypoint, []string{"/bin/app"}) {
		t.Fatalf("Entrypoint = %#v", cfg.Entrypoint)
	}
	if !reflect.DeepEqual(cfg.Cmd, []string{"serve", "--port=8080"}) {
		t.Fatalf("Cmd = %#v", cfg.Cmd)
	}
}

func TestParseImageRuntimeConfigRejectsNULInCommand(t *testing.T) {
	_, err := parseImageRuntimeConfig([]byte("{\"config\":{\"Entrypoint\":[\"/bin/app\\u0000bad\"]}}"))
	if err == nil || !strings.Contains(err.Error(), "contains NUL") {
		t.Fatalf("error = %v, want NUL rejection", err)
	}
}
