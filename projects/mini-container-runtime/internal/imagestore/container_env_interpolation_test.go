package imagestore

import (
	"reflect"
	"strings"
	"testing"
)

func TestExpandEnvString_Basic(t *testing.T) {
	env := map[string]string{
		"BASE": "/usr/local",
		"APP":  "myapp",
	}

	tests := []struct {
		input string
		want  string
	}{
		{"$BASE/bin", "/usr/local/bin"},
		{"${BASE}/lib/${APP}", "/usr/local/lib/myapp"},
		{"$$PATH", "$PATH"},
		{"$$$APP", "$myapp"},
		{"plain_string", "plain_string"},
	}

	for _, tc := range tests {
		t.Run(tc.input, func(t *testing.T) {
			got := ExpandEnvString(tc.input, env)
			if got != tc.want {
				t.Errorf("ExpandEnvString(%q) = %q, want %q", tc.input, got, tc.want)
			}
		})
	}
}

func TestExpandEnvString_POSIXExpansions(t *testing.T) {
	env := map[string]string{
		"SET_VAL":   "active",
		"EMPTY_VAL": "",
	}

	tests := []struct {
		name  string
		input string
		want  string
	}{
		{"default if unset (unset)", "${UNSET:-fallback}", "fallback"},
		{"default if unset or empty (empty)", "${EMPTY_VAL:-fallback}", "fallback"},
		{"default if unset only (empty preserved)", "${EMPTY_VAL-fallback}", ""},
		{"default if unset only (unset used)", "${UNSET-fallback}", "fallback"},
		{"alternate if set (set)", "${SET_VAL:+enabled}", "enabled"},
		{"alternate if set (empty ignored)", "${EMPTY_VAL:+enabled}", ""},
		{"alternate if set only (empty used)", "${EMPTY_VAL+enabled}", "enabled"},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := ExpandEnvString(tc.input, env)
			if got != tc.want {
				t.Errorf("got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestResolveImageEnvironment(t *testing.T) {
	configJSON := `{
		"config": {
			"Env": [
				"ROOT=/opt",
				"APP_DIR=${ROOT}/app",
				"BIN=$APP_DIR/bin",
				"PORT=${CUSTOM_PORT:-8080}",
				"DATA_DIR=${UNSET_DIR-/var/data}"
			]
		}
	}`

	resolved, err := ResolveImageEnvironment([]byte(configJSON))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	want := []string{
		"ROOT=/opt",
		"APP_DIR=/opt/app",
		"BIN=/opt/app/bin",
		"PORT=8080",
		"DATA_DIR=/var/data",
	}

	if !reflect.DeepEqual(resolved, want) {
		t.Fatalf("got %#v, want %#v", resolved, want)
	}
}

func TestFormatResolvedEnvironment(t *testing.T) {
	configJSON := `{"config":{"Env":["A=1","B=$A/2"]}}`
	got := FormatResolvedEnvironment([]byte(configJSON))
	if !strings.Contains(got, "Environment: 2 variables") {
		t.Errorf("expected header in %q", got)
	}
	if !strings.Contains(got, "B=1/2") {
		t.Errorf("expected resolved value in %q", got)
	}
}

func TestFormatResolvedEnvironment_Empty(t *testing.T) {
	got := FormatResolvedEnvironment([]byte(`{"config":{"Env":[]}}`))
	if got != "Environment: (none declared)" {
		t.Errorf("got %q, want 'Environment: (none declared)'", got)
	}
}
