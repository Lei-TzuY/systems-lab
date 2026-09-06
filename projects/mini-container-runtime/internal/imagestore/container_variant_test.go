package imagestore

import (
	"testing"
)

func TestExtractArchitectureVariant(t *testing.T) {
	tests := []struct {
		name        string
		json        string
		wantArch    string
		wantVariant string
		wantErr     bool
	}{
		{
			name:        "arm v7",
			json:        `{"architecture":"arm","variant":"v7"}`,
			wantArch:    "arm",
			wantVariant: "v7",
			wantErr:     false,
		},
		{
			name:        "arm64 v8",
			json:        `{"architecture":"arm64","variant":"v8"}`,
			wantArch:    "arm64",
			wantVariant: "v8",
			wantErr:     false,
		},
		{
			name:        "default fallback (empty config)",
			json:        `{"config":{}}`,
			wantArch:    "amd64",
			wantVariant: "",
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
			info, err := ExtractArchitectureVariant([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if info.Architecture != tc.wantArch || info.Variant != tc.wantVariant {
				t.Errorf("got (%s, %s), want (%s, %s)",
					info.Architecture, info.Variant, tc.wantArch, tc.wantVariant)
			}
		})
	}
}

func TestFormatArchitectureVariant(t *testing.T) {
	t.Run("with variant", func(t *testing.T) {
		jsonBlob := `{"architecture":"arm","variant":"v7"}`
		got := FormatArchitectureVariant([]byte(jsonBlob))
		want := "Arch: arm (variant: v7)"
		if got != want {
			t.Errorf("got %q, want %q", got, want)
		}
	})

	t.Run("without variant", func(t *testing.T) {
		jsonBlob := `{"architecture":"amd64"}`
		got := FormatArchitectureVariant([]byte(jsonBlob))
		want := "Arch: amd64"
		if got != want {
			t.Errorf("got %q, want %q", got, want)
		}
	})
}
