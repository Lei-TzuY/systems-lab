//go:build linux

package network

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestIPAMRejectsSemanticallyInvalidAllocations(t *testing.T) {
	tests := []struct {
		name string
		json string
		want string
	}{
		{
			name: "outside subnet",
			json: `{"subnet":"10.80.0.0/29","allocated":{"10.81.0.2":"ctr-a"}}`,
			want: "outside subnet",
		},
		{
			name: "gateway reserved",
			json: `{"subnet":"10.80.0.0/29","allocated":{"10.80.0.1":"ctr-a"}}`,
			want: "reserved subnet address",
		},
		{
			name: "broadcast reserved",
			json: `{"subnet":"10.80.0.0/29","allocated":{"10.80.0.7":"ctr-a"}}`,
			want: "reserved subnet address",
		},
		{
			name: "empty owner",
			json: `{"subnet":"10.80.0.0/29","allocated":{"10.80.0.2":""}}`,
			want: "invalid owner",
		},
		{
			name: "duplicate owner",
			json: `{"subnet":"10.80.0.0/29","allocated":{"10.80.0.2":"ctr-a","10.80.0.3":"ctr-a"}}`,
			want: "more than one address",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			dir := t.TempDir()
			ipam, err := OpenIPAM(dir)
			if err != nil {
				t.Fatal(err)
			}
			fixture := strings.ReplaceAll(tt.json, `\"`, `"`)
			if err := os.WriteFile(filepath.Join(dir, "bad.json"), []byte(fixture), 0600); err != nil {
				t.Fatal(err)
			}
			if _, err := ipam.AllocateIP("bad", "10.80.0.0/29", "ctr-new"); err == nil || !strings.Contains(err.Error(), tt.want) {
				t.Fatalf("error=%v, want substring %q", err, tt.want)
			}
		})
	}
}
