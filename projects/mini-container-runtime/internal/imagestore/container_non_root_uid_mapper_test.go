package imagestore

import (
	"strings"
	"testing"
)

func TestValidateUserNamespaceMapping(t *testing.T) {
	subRange := SubIDRange{StartID: 100000, Length: 65536}

	tests := []struct {
		name         string
		json         string
		subRange     SubIDRange
		wantUID      int64
		wantGID      int64
		wantRoot     bool
		wantRootless bool
		wantErr      bool
	}{
		{
			name:         "numeric non-root user 1000:1000",
			json:         `{"config":{"User":"1000:1000"}}`,
			subRange:     subRange,
			wantUID:      1000,
			wantGID:      1000,
			wantRoot:     false,
			wantRootless: true,
		},
		{
			name:         "root user default",
			json:         `{"config":{"User":"root"}}`,
			subRange:     subRange,
			wantUID:      0,
			wantGID:      0,
			wantRoot:     true,
			wantRootless: true,
		},
		{
			name:         "compound root user root:0",
			json:         `{"config":{"User":"root:0"}}`,
			subRange:     subRange,
			wantUID:      0,
			wantGID:      0,
			wantRoot:     true,
			wantRootless: true,
		},
		{
			name:         "compound root user root:wheel",
			json:         `{"config":{"User":"root:wheel"}}`,
			subRange:     subRange,
			wantUID:      0,
			wantGID:      65534,
			wantRoot:     true,
			wantRootless: true,
		},
		{
			name:         "compound root user root:1000",
			json:         `{"config":{"User":"root:1000"}}`,
			subRange:     subRange,
			wantUID:      0,
			wantGID:      1000,
			wantRoot:     true,
			wantRootless: true,
		},
		{
			name:         "compound root user 0:root",
			json:         `{"config":{"User":"0:root"}}`,
			subRange:     subRange,
			wantUID:      0,
			wantGID:      0,
			wantRoot:     true,
			wantRootless: true,
		},
		{
			name:         "compound root user 0:1000",
			json:         `{"config":{"User":"0:1000"}}`,
			subRange:     subRange,
			wantUID:      0,
			wantGID:      1000,
			wantRoot:     true,
			wantRootless: true,
		},
		{
			name:         "symbolic user nobody with root group",
			json:         `{"config":{"User":"nobody:root"}}`,
			subRange:     subRange,
			wantUID:      65534,
			wantGID:      0,
			wantRoot:     false,
			wantRootless: true,
		},
		{
			name:         "symbolic user nobody",
			json:         `{"config":{"User":"nobody"}}`,
			subRange:     subRange,
			wantUID:      65534,
			wantGID:      65534,
			wantRoot:     false,
			wantRootless: true,
		},
		{
			name:         "zero length subuid range rejects non-rootless",
			json:         `{"config":{"User":"1000"}}`,
			subRange:     SubIDRange{StartID: 0, Length: 0},
			wantUID:      1000,
			wantGID:      0,
			wantRoot:     false,
			wantRootless: false,
		},
		{
			name:     "negative UID is rejected",
			json:     `{"config":{"User":"-1:1000"}}`,
			subRange: subRange,
			wantErr:  true,
		},
		{
			name:     "negative GID is rejected",
			json:     `{"config":{"User":"1000:-5"}}`,
			subRange: subRange,
			wantErr:  true,
		},
		{
			name:     "symbolic user with negative GID is rejected",
			json:     `{"config":{"User":"nobody:-5"}}`,
			subRange: subRange,
			wantErr:  true,
		},
		{
			name:     "UID exceeding POSIX max is rejected",
			json:     `{"config":{"User":"5000000000"}}`,
			subRange: subRange,
			wantErr:  true,
		},
		{
			name:     "GID exceeding POSIX max is rejected",
			json:     `{"config":{"User":"1000:5000000000"}}`,
			subRange: subRange,
			wantErr:  true,
		},
		{
			name:     "too many colons is rejected",
			json:     `{"config":{"User":"1000:1000:extra"}}`,
			subRange: subRange,
			wantErr:  true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			res, err := ValidateUserNamespaceMapping([]byte(tc.json), tc.subRange)
			if tc.wantErr {
				if err == nil {
					t.Fatalf("expected error, got %+v", res)
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if res.UID != tc.wantUID {
				t.Errorf("UID = %d, want %d", res.UID, tc.wantUID)
			}
			if res.GID != tc.wantGID {
				t.Errorf("GID = %d, want %d", res.GID, tc.wantGID)
			}
			if res.IsRoot != tc.wantRoot {
				t.Errorf("IsRoot = %t, want %t", res.IsRoot, tc.wantRoot)
			}
			if res.IsRootlessAllowed != tc.wantRootless {
				t.Errorf("IsRootlessAllowed = %t, want %t", res.IsRootlessAllowed, tc.wantRootless)
			}
		})
	}
}

func TestFormatUserNamespaceMapping(t *testing.T) {
	subRange := SubIDRange{StartID: 100000, Length: 65536}
	got := FormatUserNamespaceMapping([]byte(`{"config":{"User":"1000:1000"}}`), subRange)
	if !strings.Contains(got, "User Namespace Mapping Evaluation:") {
		t.Errorf("expected evaluation header in %q", got)
	}
	if !strings.Contains(got, "Parsed UID:GID: 1000:1000") {
		t.Errorf("expected parsed UID:GID in %q", got)
	}
}

func TestFormatUserNamespaceMapping_InvalidJSON(t *testing.T) {
	got := FormatUserNamespaceMapping([]byte("invalid json"), SubIDRange{})
	if !strings.Contains(got, "error: parse image config for user mapping") {
		t.Errorf("expected error in %q", got)
	}
}
