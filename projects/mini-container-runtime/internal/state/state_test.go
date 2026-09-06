// internal/state/state_test.go

package state

import (
	"fmt"
	"os"
	"testing"
	"time"
)

func TestSaveAndLoad(t *testing.T) {
	dir := t.TempDir()
	store, err := Open(dir)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}

	c := &Container{
		ID:        "aabbccdd",
		PID:       12345,
		Status:    StatusRunning,
		RootFS:    "/tmp/rootfs",
		Command:   []string{"/bin/sh"},
		Hostname:  "test",
		CreatedAt: time.Now().Truncate(time.Second),
	}
	if err := store.Save(c); err != nil {
		t.Fatalf("Save: %v", err)
	}

	got, err := store.Load("aabbccdd")
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	if got.PID != 12345 {
		t.Errorf("PID = %d, want 12345", got.PID)
	}
	if got.Status != StatusRunning {
		t.Errorf("Status = %q, want running", got.Status)
	}
}

func TestList(t *testing.T) {
	dir := t.TempDir()
	store, err := Open(dir)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}

	for _, id := range []string{"aaa1", "bbb2", "ccc3"} {
		if err := store.Save(&Container{ID: id, Status: StatusStopped}); err != nil {
			t.Fatalf("Save %s: %v", id, err)
		}
	}

	all, err := store.List()
	if err != nil {
		t.Fatalf("List: %v", err)
	}
	if len(all) != 3 {
		t.Errorf("List returned %d records, want 3", len(all))
	}
}

func TestResolvePrefix(t *testing.T) {
	dir := t.TempDir()
	store, err := Open(dir)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	_ = store.Save(&Container{ID: "aabbccdd1122", Status: StatusRunning})
	_ = store.Save(&Container{ID: "eeff00112233", Status: StatusStopped})

	c, err := store.Resolve("aabb")
	if err != nil {
		t.Fatalf("Resolve: %v", err)
	}
	if c.ID != "aabbccdd1122" {
		t.Errorf("ID = %q, want aabbccdd1122", c.ID)
	}

	// Ambiguous prefix
	if _, err := store.Resolve(""); err == nil {
		t.Error("empty prefix should be ambiguous")
	}

	// Unknown prefix
	if _, err := store.Resolve("zzzz"); err == nil {
		t.Error("unknown prefix should error")
	}
}

func TestDelete(t *testing.T) {
	dir := t.TempDir()
	store, err := Open(dir)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	_ = store.Save(&Container{ID: "deadbeef"})
	if err := store.Delete("deadbeef"); err != nil {
		t.Fatalf("Delete: %v", err)
	}
	if _, err := store.Load("deadbeef"); err == nil {
		t.Error("Load after Delete should error")
	}
	// Idempotent second delete
	if err := store.Delete("deadbeef"); err != nil {
		t.Errorf("second Delete returned error: %v", err)
	}
}

func TestNewID(t *testing.T) {
	seen := map[string]bool{}
	for i := 0; i < 100; i++ {
		id, err := NewID()
		if err != nil {
			t.Fatalf("NewID: %v", err)
		}
		if len(id) != 16 {
			t.Errorf("len(id) = %d, want 16", len(id))
		}
		if seen[id] {
			t.Errorf("duplicate ID: %s", id)
		}
		seen[id] = true
	}
}

func TestSaveImageAndList(t *testing.T) {
	dir := t.TempDir()
	store, err := Open(dir)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}

	img := &Image{
		Name:     "alpine:3.19",
		RootFS:   "/tmp/rootfs-alpine",
		LoadedAt: time.Now(),
	}
	if err := store.SaveImage(img); err != nil {
		t.Fatalf("SaveImage: %v", err)
	}

	imgs, err := store.ListImages()
	if err != nil {
		t.Fatalf("ListImages: %v", err)
	}
	if len(imgs) != 1 {
		t.Fatalf("ListImages returned %d, want 1", len(imgs))
	}
	if imgs[0].Name != "alpine:3.19" {
		t.Errorf("Name = %q", imgs[0].Name)
	}
}

func TestOpenCreatesDirectories(t *testing.T) {
	dir := t.TempDir()
	subdir := dir + "/does/not/exist"
	_, err := Open(subdir)
	if err != nil {
		t.Fatalf("Open should create directories: %v", err)
	}
	for _, sub := range []string{"containers", "images"} {
		if _, err := os.Stat(subdir + "/" + sub); os.IsNotExist(err) {
			t.Errorf("directory %q not created", sub)
		}
	}
}

func TestStateStoreTraversalDefense(t *testing.T) {
	dir := t.TempDir()
	store, err := Open(dir)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}

	traversalIDs := []string{
		"",
		".",
		"..",
		"../escape",
		"../../etc/passwd",
		"foo/bar",
		"foo\\bar",
		"colon:id",
	}

	for _, id := range traversalIDs {
		if err := store.Save(&Container{ID: id}); err == nil {
			t.Errorf("Save(%q) expected error, got nil", id)
		}
		if _, err := store.Get(id); err == nil {
			t.Errorf("Get(%q) expected error, got nil", id)
		}
		if err := store.Delete(id); err == nil {
			t.Errorf("Delete(%q) expected error, got nil", id)
		}
		if _, err := store.Resolve(id); err == nil {
			t.Errorf("Resolve(%q) expected error, got nil", id)
		}
	}
}

func TestSanitizeImageFilename(t *testing.T) {
	tests := []struct {
		input string
		want  string
	}{
		{"alpine:3.19", "alpine_3.19"},
		{"registry.example.com/user/app:v1", "registry.example.com_user_app_v1"},
		{"../../evil", "evil"},
		{"", "default"},
		{"...", "default"},
		{".", "default"},
	}

	for _, tt := range tests {
		got := sanitizeImageFilename(tt.input)
		if got != tt.want {
			t.Errorf("sanitizeImageFilename(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

func TestConcurrentAtomicSaves(t *testing.T) {
	dir := t.TempDir()
	store, err := Open(dir)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}

	const goroutines = 20
	const iterations = 20
	errCh := make(chan error, goroutines)

	for g := 0; g < goroutines; g++ {
		go func(gid int) {
			// Each goroutine creates its own store instance pointing to the same dir
			// to simulate independent concurrent processes writing distinct records.
			localStore, err := Open(dir)
			if err != nil {
				errCh <- err
				return
			}

			ctrID := fmt.Sprintf("ctr-concurrent-%d", gid)
			c := &Container{ID: ctrID, Status: StatusRunning}
			for i := 0; i < iterations; i++ {
				c.PID = gid*1000 + i
				if err := localStore.Save(c); err != nil {
					errCh <- fmt.Errorf("Save error on gid %d iter %d: %w", gid, i, err)
					return
				}
				if c.Revision != uint64(i+1) {
					errCh <- fmt.Errorf("revision on gid %d iter %d = %d, want %d", gid, i, c.Revision, i+1)
					return
				}
				loaded, err := localStore.Get(ctrID)
				if err != nil {
					errCh <- fmt.Errorf("Get error on gid %d iter %d: %w", gid, i, err)
					return
				}
				if loaded.ID != ctrID {
					errCh <- fmt.Errorf("loaded ID %q != want %q", loaded.ID, ctrID)
					return
				}
			}
			errCh <- nil
		}(g)
	}

	for g := 0; g < goroutines; g++ {
		if err := <-errCh; err != nil {
			t.Fatalf("Concurrent save failed: %v", err)
		}
	}

	all, err := store.List()
	if err != nil {
		t.Fatalf("List error: %v", err)
	}
	if len(all) != goroutines {
		t.Fatalf("List returned %d containers, want %d", len(all), goroutines)
	}
}

func TestConcurrentSaveImage(t *testing.T) {
	dir := t.TempDir()
	const goroutines = 15
	const iterations = 10
	errCh := make(chan error, goroutines)

	for g := 0; g < goroutines; g++ {
		go func(gid int) {
			localStore, err := Open(dir)
			if err != nil {
				errCh <- err
				return
			}

			imgName := fmt.Sprintf("image-%d:v1", gid)
			for i := 0; i < iterations; i++ {
				img := &Image{
					ID:       fmt.Sprintf("img-id-%d", gid),
					Name:     imgName,
					RootFS:   fmt.Sprintf("/rootfs/%d-%d", gid, i),
					LoadedAt: time.Now(),
				}
				if err := localStore.SaveImage(img); err != nil {
					errCh <- fmt.Errorf("SaveImage gid %d iter %d: %w", gid, i, err)
					return
				}
				loaded, err := localStore.GetImage(imgName)
				if err != nil {
					errCh <- fmt.Errorf("GetImage gid %d iter %d: %w", gid, i, err)
					return
				}
				if loaded.Name != imgName {
					errCh <- fmt.Errorf("loaded Name %q != want %q", loaded.Name, imgName)
					return
				}
			}
			errCh <- nil
		}(g)
	}

	for g := 0; g < goroutines; g++ {
		if err := <-errCh; err != nil {
			t.Fatalf("Concurrent SaveImage failed: %v", err)
		}
	}
}
