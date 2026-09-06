package imagestore

import (
	"testing"
)

func TestExtractTTYFlags(t *testing.T) {
	tests := []struct {
		name          string
		json          string
		wantTty       bool
		wantOpenStdin bool
		wantStdinOnce bool
		wantErr       bool
	}{
		{
			name:          "interactive terminal enabled",
			json:          `{"config":{"Tty":true,"OpenStdin":true,"StdinOnce":false}}`,
			wantTty:       true,
			wantOpenStdin: true,
			wantStdinOnce: false,
			wantErr:       false,
		},
		{
			name:          "all false by default",
			json:          `{"config":{}}`,
			wantTty:       false,
			wantOpenStdin: false,
			wantStdinOnce: false,
			wantErr:       false,
		},
		{
			name:          "stdin once enabled",
			json:          `{"config":{"Tty":false,"OpenStdin":true,"StdinOnce":true}}`,
			wantTty:       false,
			wantOpenStdin: true,
			wantStdinOnce: true,
			wantErr:       false,
		},
		{
			name:    "invalid json",
			json:    `{invalid`,
			wantErr: true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			flags, err := ExtractTTYFlags([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if flags.Tty != tc.wantTty || flags.OpenStdin != tc.wantOpenStdin || flags.StdinOnce != tc.wantStdinOnce {
				t.Errorf("ExtractTTYFlags() = %+v, want tty=%t, openStdin=%t, stdinOnce=%t",
					flags, tc.wantTty, tc.wantOpenStdin, tc.wantStdinOnce)
			}
		})
	}
}

func TestFormatTTYFlags(t *testing.T) {
	jsonBlob := `{"config":{"Tty":true,"OpenStdin":true,"StdinOnce":false}}`
	got := FormatTTYFlags([]byte(jsonBlob))
	want := "TTY/Stdin: tty=true, open_stdin=true, stdin_once=false"
	if got != want {
		t.Errorf("FormatTTYFlags() = %q, want %q", got, want)
	}

	errOut := FormatTTYFlags([]byte(`{bad`))
	if errOut == "" || errOut == want {
		t.Errorf("expected error message, got %q", errOut)
	}
}
