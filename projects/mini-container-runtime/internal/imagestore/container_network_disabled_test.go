package imagestore

import (
	"testing"
)

func TestExtractNetworkSettings(t *testing.T) {
	tests := []struct {
		name         string
		json         string
		wantDisabled bool
		wantMac      string
		wantErr      bool
	}{
		{
			name:         "network disabled with fixed MAC",
			json:         `{"config":{"NetworkDisabled":true,"MacAddress":"02:42:ac:11:00:02"}}`,
			wantDisabled: true,
			wantMac:      "02:42:ac:11:00:02",
			wantErr:      false,
		},
		{
			name:         "empty defaults",
			json:         `{"config":{}}`,
			wantDisabled: false,
			wantMac:      "",
			wantErr:      false,
		},
		{
			name:    "invalid json",
			json:    `{invalid`,
			wantErr: true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			settings, err := ExtractNetworkSettings([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if settings.NetworkDisabled != tc.wantDisabled || settings.MacAddress != tc.wantMac {
				t.Errorf("ExtractNetworkSettings() = %+v, want disabled=%t, mac=%s",
					settings, tc.wantDisabled, tc.wantMac)
			}
		})
	}
}

func TestFormatNetworkSettings(t *testing.T) {
	t.Run("with MAC", func(t *testing.T) {
		jsonBlob := `{"config":{"NetworkDisabled":true,"MacAddress":"02:42:ac:11:00:02"}}`
		got := FormatNetworkSettings([]byte(jsonBlob))
		want := "Network: disabled=true, mac=02:42:ac:11:00:02"
		if got != want {
			t.Errorf("got %q, want %q", got, want)
		}
	})

	t.Run("dynamic MAC", func(t *testing.T) {
		jsonBlob := `{"config":{"NetworkDisabled":false}}`
		got := FormatNetworkSettings([]byte(jsonBlob))
		want := "Network: disabled=false, mac=(dynamic)"
		if got != want {
			t.Errorf("got %q, want %q", got, want)
		}
	})

	t.Run("invalid json", func(t *testing.T) {
		got := FormatNetworkSettings([]byte(`{bad`))
		if got == "" {
			t.Error("expected error message")
		}
	})
}
