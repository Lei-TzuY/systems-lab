package imagestore

import (
	"testing"
)

func TestExtractAttachFlags(t *testing.T) {
	tests := []struct {
		name       string
		json       string
		wantStdin  bool
		wantStdout bool
		wantStderr bool
		wantErr    bool
	}{
		{
			name:       "all enabled",
			json:       `{"config":{"AttachStdin":true,"AttachStdout":true,"AttachStderr":true}}`,
			wantStdin:  true,
			wantStdout: true,
			wantStderr: true,
			wantErr:    false,
		},
		{
			name:       "stdout and stderr only",
			json:       `{"config":{"AttachStdin":false,"AttachStdout":true,"AttachStderr":true}}`,
			wantStdin:  false,
			wantStdout: true,
			wantStderr: true,
			wantErr:    false,
		},
		{
			name:       "empty config defaults to false",
			json:       `{"config":{}}`,
			wantStdin:  false,
			wantStdout: false,
			wantStderr: false,
			wantErr:    false,
		},
		{
			name:    "invalid json",
			json:    `{bad_json`,
			wantErr: true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			flags, err := ExtractAttachFlags([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if flags.Stdin != tc.wantStdin || flags.Stdout != tc.wantStdout || flags.Stderr != tc.wantStderr {
				t.Errorf("ExtractAttachFlags() = %+v, want stdin=%t, stdout=%t, stderr=%t",
					flags, tc.wantStdin, tc.wantStdout, tc.wantStderr)
			}
		})
	}
}

func TestFormatAttachFlags(t *testing.T) {
	jsonBlob := `{"config":{"AttachStdin":false,"AttachStdout":true,"AttachStderr":true}}`
	got := FormatAttachFlags([]byte(jsonBlob))
	want := "Attach: stdin=false, stdout=true, stderr=true"
	if got != want {
		t.Errorf("FormatAttachFlags() = %q, want %q", got, want)
	}

	errOut := FormatAttachFlags([]byte(`{bad`))
	if errOut == "" || errOut == want {
		t.Errorf("expected error output, got %q", errOut)
	}
}
