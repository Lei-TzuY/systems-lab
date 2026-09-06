package events

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
	"time"

	"minicontainer/internal/state"
)

const createGuardTestID = "0123456789abcdef"

func TestPublishCreateRequiresDurableExactState(t *testing.T) {
	t.Setenv("HOME", t.TempDir())

	if err := Publish(EventCreate, createGuardTestID, "/rootfs", "created container"); err == nil {
		t.Fatal("Publish(EventCreate) succeeded without durable container state")
	}
	if _, err := os.Stat(LogPath()); !os.IsNotExist(err) {
		t.Fatalf("events log exists after rejected create: %v", err)
	}
}

func TestPublishCreateUsesExactContainerIdentity(t *testing.T) {
	t.Setenv("HOME", t.TempDir())
	persistCreateGuardContainer(t, createGuardTestID)

	if err := Publish(EventCreate, createGuardTestID[:8], "/rootfs", "created container"); err == nil {
		t.Fatal("Publish(EventCreate) accepted a prefix instead of exact durable state")
	}
	if _, err := os.Stat(LogPath()); !os.IsNotExist(err) {
		t.Fatalf("events log exists after rejected prefix create: %v", err)
	}
}

func TestPublishCreateAfterDurableState(t *testing.T) {
	t.Setenv("HOME", t.TempDir())
	persistCreateGuardContainer(t, createGuardTestID)

	if err := Publish(EventCreate, createGuardTestID, "/rootfs", "created container"); err != nil {
		t.Fatalf("Publish(EventCreate): %v", err)
	}

	data, err := os.ReadFile(LogPath())
	if err != nil {
		t.Fatalf("read events log: %v", err)
	}
	var evt Event
	if err := json.Unmarshal(data, &evt); err != nil {
		t.Fatalf("decode create event: %v", err)
	}
	if evt.Type != EventCreate || evt.ContainerID != createGuardTestID {
		t.Fatalf("event=%+v, want create for %s", evt, createGuardTestID)
	}
}

func TestPublishCreateRejectsReplacedStateRoot(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	persistCreateGuardContainer(t, createGuardTestID)

	stateRoot := state.DefaultDir()
	oldRoot := filepath.Join(home, ".minicontainer-old")
	if err := os.Rename(stateRoot, oldRoot); err != nil {
		t.Fatalf("rename state root: %v", err)
	}
	if err := os.Mkdir(stateRoot, 0o700); err != nil {
		t.Fatalf("create replacement state root: %v", err)
	}

	if err := Publish(EventCreate, createGuardTestID, "/rootfs", "created container"); err == nil {
		t.Fatal("Publish(EventCreate) accepted state from replaced pathname generation")
	}
	if _, err := os.Stat(LogPath()); !os.IsNotExist(err) {
		t.Fatalf("replacement state root received create event: %v", err)
	}
}

func persistCreateGuardContainer(t *testing.T, id string) {
	t.Helper()
	st, err := state.Open(state.DefaultDir())
	if err != nil {
		t.Fatalf("open state store: %v", err)
	}
	defer st.Close()

	rec := &state.Container{
		ID:        id,
		Status:    state.StatusCreated,
		RootFS:    "/rootfs",
		Command:   []string{"/bin/true"},
		Hostname:  "minicontainer",
		CreatedAt: time.Now(),
	}
	if err := st.Save(rec); err != nil {
		t.Fatalf("save container state: %v", err)
	}
}
