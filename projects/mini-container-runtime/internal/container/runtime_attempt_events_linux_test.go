//go:build linux

package container

import (
	"errors"
	"os"
	"testing"

	"minicontainer/internal/events"
)

func TestManagedAttemptAdmissionRollsBackUncommittedStart(t *testing.T) {
	t.Setenv("HOME", t.TempDir())

	cfg := Config{
		ContainerID: "ctr-attempt-event-rollback",
		RootFS:      "/tmp/rootfs",
	}
	rollback, err := beginNetworkAttemptAdmission(cfg, nil)
	if err != nil {
		t.Fatalf("attempt admission: %v", err)
	}
	if rollback == nil {
		t.Fatal("managed attempt admission returned nil rollback")
	}
	if err := rollback(); err != nil {
		t.Fatalf("rollback: %v", err)
	}
	if err := events.CommitPendingStart(); err != nil {
		t.Fatalf("commit after rollback: %v", err)
	}
	if _, err := os.Stat(events.LogPath()); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("pre-exec rollback produced a start event: %v", err)
	}
}
