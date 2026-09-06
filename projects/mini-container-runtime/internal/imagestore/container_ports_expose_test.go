package imagestore

import (
	"reflect"
	"testing"
)

func TestExtractExposedPorts(t *testing.T) {
	tests := []struct {
		name         string
		json         string
		wantTotal    int
		wantTCPPorts []int
		wantUDPPorts []int
		wantErr      bool
	}{
		{
			name: "mixed tcp and udp and sctp ports",
			json: `{
				"config": {
					"ExposedPorts": {
						"80/tcp": {},
						"443/tcp": {},
						"53/udp": {},
						"9000/sctp": {},
						"8080": {}
					}
				}
			}`,
			wantTotal:    5,
			wantTCPPorts: []int{80, 443, 8080},
			wantUDPPorts: []int{53},
			wantErr:      false,
		},
		{
			name:         "empty exposed ports",
			json:         `{"config": {"ExposedPorts": {}}}`,
			wantTotal:    0,
			wantTCPPorts: nil,
			wantUDPPorts: nil,
			wantErr:      false,
		},
		{
			name:         "missing config field",
			json:         `{"config": {}}`,
			wantTotal:    0,
			wantTCPPorts: nil,
			wantUDPPorts: nil,
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
			summary, err := ExtractExposedPorts([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if summary.TotalPorts != tc.wantTotal {
				t.Errorf("TotalPorts = %d, want %d", summary.TotalPorts, tc.wantTotal)
			}
			if !reflect.DeepEqual(summary.TCPPorts, tc.wantTCPPorts) {
				t.Errorf("TCPPorts = %v, want %v", summary.TCPPorts, tc.wantTCPPorts)
			}
			if !reflect.DeepEqual(summary.UDPPorts, tc.wantUDPPorts) {
				t.Errorf("UDPPorts = %v, want %v", summary.UDPPorts, tc.wantUDPPorts)
			}
		})
	}
}

func TestFormatExposedPorts(t *testing.T) {
	t.Run("formatted list", func(t *testing.T) {
		jsonBlob := `{"config":{"ExposedPorts":{"80/tcp":{},"53/udp":{}}}}`
		got := FormatExposedPorts([]byte(jsonBlob))
		want := "Exposed Ports (2): 53/udp, 80/tcp"
		if got != want {
			t.Errorf("got %q, want %q", got, want)
		}
	})

	t.Run("none", func(t *testing.T) {
		got := FormatExposedPorts([]byte(`{"config":{}}`))
		if got != "Exposed Ports: none" {
			t.Errorf("got %q", got)
		}
	})
}
