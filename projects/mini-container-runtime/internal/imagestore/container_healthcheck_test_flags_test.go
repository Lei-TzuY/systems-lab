package imagestore

import (
	"reflect"
	"testing"
)

func TestExtractHealthcheckTest(t *testing.T) {
	tests := []struct {
		name        string
		json        string
		wantType    HealthcheckType
		wantCommand []string
		wantErr     bool
	}{
		{
			name:        "CMD format",
			json:        `{"config":{"Healthcheck":{"Test":["CMD","curl","-f","http://localhost/"]}}}`,
			wantType:    HealthcheckCmd,
			wantCommand: []string{"curl", "-f", "http://localhost/"},
			wantErr:     false,
		},
		{
			name:        "CMD-SHELL format",
			json:        `{"config":{"Healthcheck":{"Test":["CMD-SHELL","pg_isready -h localhost"]}}}`,
			wantType:    HealthcheckCmdShell,
			wantCommand: []string{"pg_isready -h localhost"},
			wantErr:     false,
		},
		{
			name:        "NONE format (disabled)",
			json:        `{"config":{"Healthcheck":{"Test":["NONE"]}}}`,
			wantType:    HealthcheckNone,
			wantCommand: nil,
			wantErr:     false,
		},
		{
			name:        "direct command without prefix",
			json:        `{"config":{"Healthcheck":{"Test":["/bin/check.sh","--fast"]}}}`,
			wantType:    HealthcheckCmd,
			wantCommand: []string{"/bin/check.sh", "--fast"},
			wantErr:     false,
		},
		{
			name:        "undefined healthcheck",
			json:        `{"config":{}}`,
			wantType:    HealthcheckUndefined,
			wantCommand: nil,
			wantErr:     false,
		},
		{
			name:    "invalid json",
			json:    `{invalid`,
			wantErr: true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			info, err := ExtractHealthcheckTest([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if info.Type != tc.wantType {
				t.Errorf("Type = %v, want %v", info.Type, tc.wantType)
			}
			if !reflect.DeepEqual(info.Command, tc.wantCommand) {
				t.Errorf("Command = %v, want %v", info.Command, tc.wantCommand)
			}
		})
	}
}

func TestFormatHealthcheckTest(t *testing.T) {
	t.Run("NONE", func(t *testing.T) {
		got := FormatHealthcheckTest([]byte(`{"config":{"Healthcheck":{"Test":["NONE"]}}}`))
		want := "Healthcheck: NONE (disabled)"
		if got != want {
			t.Errorf("got %q, want %q", got, want)
		}
	})

	t.Run("CMD-SHELL", func(t *testing.T) {
		got := FormatHealthcheckTest([]byte(`{"config":{"Healthcheck":{"Test":["CMD-SHELL","curl -f localhost"]}}}`))
		want := "Healthcheck: CMD-SHELL curl -f localhost"
		if got != want {
			t.Errorf("got %q, want %q", got, want)
		}
	})
}
