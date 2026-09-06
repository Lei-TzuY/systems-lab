package registry

import "testing"

func TestParseImageRef(t *testing.T) {
	tests := []struct {
		in       string
		wantName string
		wantTag  string
	}{
		{"alpine", "library/alpine", "latest"},
		{"alpine:3.19", "library/alpine", "3.19"},
		{"ubuntu:22.04", "library/ubuntu", "22.04"},
		{"myorg/myapp", "myorg/myapp", "latest"},
		{"myorg/myapp:v1.0", "myorg/myapp", "v1.0"},
	}

	for _, tt := range tests {
		t.Run(tt.in, func(t *testing.T) {
			name, tag := parseImageRef(tt.in)
			if name != tt.wantName || tag != tt.wantTag {
				t.Errorf("parseImageRef(%q) = (%q, %q), want (%q, %q)",
					tt.in, name, tag, tt.wantName, tt.wantTag)
			}
		})
	}
}
