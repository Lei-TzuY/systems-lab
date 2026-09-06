package state

import (
	"strings"
	"testing"
)

func TestValidateImagePublicationAgainstRegistry(t *testing.T) {
	tests := []struct {
		name      string
		existing  []*Image
		candidate *Image
		key       string
		wantErr   string
	}{
		{
			name:      "same ID and rootfs alias is allowed",
			existing:  []*Image{{Name: "repo:old", ID: "sha256:abc", RootFS: "/rootfs/a"}},
			candidate: &Image{Name: "repo:new", ID: "sha256:abc", RootFS: "/rootfs/a"},
			key:       "repo:new",
		},
		{
			name:      "same ID with different rootfs is rejected",
			existing:  []*Image{{Name: "repo:old", ID: "sha256:abc", RootFS: "/rootfs/a"}},
			candidate: &Image{Name: "repo:new", ID: "sha256:abc", RootFS: "/rootfs/b"},
			key:       "repo:new",
			wantErr:   "already references rootfs",
		},
		{
			name:      "candidate name colliding with existing exact ID is rejected",
			existing:  []*Image{{Name: "repo:old", ID: "collision", RootFS: "/rootfs/a"}},
			candidate: &Image{Name: "collision", ID: "sha256:new", RootFS: "/rootfs/b"},
			key:       "collision",
			wantErr:   "collides with exact ID",
		},
		{
			name:      "candidate ID colliding with existing exact name is rejected",
			existing:  []*Image{{Name: "collision", ID: "sha256:old", RootFS: "/rootfs/a"}},
			candidate: &Image{Name: "repo:new", ID: "collision", RootFS: "/rootfs/b"},
			key:       "repo:new",
			wantErr:   "collides with exact name",
		},
		{
			name:      "replacement ignores superseded record",
			existing:  []*Image{{Name: "repo:tag", ID: "sha256:old", RootFS: "/rootfs/a"}},
			candidate: &Image{Name: "repo:tag", ID: "sha256:new", RootFS: "/rootfs/b"},
			key:       "repo:tag",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := validateImagePublicationAgainstRegistry(tt.existing, tt.candidate, tt.key)
			if tt.wantErr == "" {
				if err != nil {
					t.Fatalf("unexpected error: %v", err)
				}
				return
			}
			if err == nil || !strings.Contains(err.Error(), tt.wantErr) {
				t.Fatalf("error = %v, want substring %q", err, tt.wantErr)
			}
		})
	}
}
