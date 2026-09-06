package imagestore

import (
	"path/filepath"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestTagPublicationPreservesDisplacedPayloadOwnership(t *testing.T) {
	tests := []struct {
		name string
		tag  func(*state.Store, string, string) (*state.Image, error)
	}{
		{name: "TagImage", tag: TagImage},
		{name: "CreateTagAlias", tag: CreateTagAlias},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			base := t.TempDir()
			st, err := state.Open(base)
			if err != nil {
				t.Fatal(err)
			}
			defer st.Close()

			oldRoot := filepath.Join(base, "images", "old-payload", "rootfs")
			old := &state.Image{
				ID:         "old-payload",
				Name:       "app:latest",
				Repository: "app",
				Tag:        "latest",
				RootFS:     oldRoot,
				Size:       10,
				LoadedAt:   time.Now(),
			}
			newRoot := filepath.Join(base, "images", "new-payload", "rootfs")
			source := &state.Image{
				ID:         "new-payload",
				Name:       "app:v2",
				Repository: "app",
				Tag:        "v2",
				RootFS:     newRoot,
				Size:       20,
				LoadedAt:   time.Now(),
			}
			if err := st.SaveImage(old); err != nil {
				t.Fatal(err)
			}
			if err := st.SaveImage(source); err != nil {
				t.Fatal(err)
			}

			published, err := tt.tag(st, source.Name, old.Name)
			if err != nil {
				t.Fatalf("tag replacement: %v", err)
			}
			if published.ID != source.ID || filepath.Clean(published.RootFS) != filepath.Clean(source.RootFS) {
				t.Fatalf("published image=%+v, want source payload", published)
			}

			current, err := st.GetImage(old.Name)
			if err != nil {
				t.Fatalf("read replacement target: %v", err)
			}
			if current.ID != source.ID || filepath.Clean(current.RootFS) != filepath.Clean(source.RootFS) {
				t.Fatalf("replacement target=%+v, want source payload", current)
			}

			dangling, err := st.GetImage(old.ID)
			if err != nil {
				t.Fatalf("displaced payload lost after tag replacement: %v", err)
			}
			if dangling.ID != old.ID || dangling.Name != "" || dangling.Tag != "<none>" || filepath.Clean(dangling.RootFS) != filepath.Clean(old.RootFS) {
				t.Fatalf("dangling record=%+v, want displaced payload ownership", dangling)
			}

			if _, err := st.GetImage(source.Name); err != nil {
				t.Fatalf("source alias disappeared after tag replacement: %v", err)
			}
			images, err := st.ListImages()
			if err != nil {
				t.Fatal(err)
			}
			if len(images) != 3 {
				t.Fatalf("image records=%d, want source + replacement + dangling", len(images))
			}
		})
	}
}
