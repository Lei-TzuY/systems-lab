package main

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/container"
	"minicontainer/internal/state"
)

const runAdmissionTestID = "0123456789abcdef"

func TestPrepareManagedRunStateFailsClosedWhenStoreOpenFails(t *testing.T) {
	cause := errors.New("state unavailable")
	cfg := runAdmissionTestConfig(t)
	openCalls := 0

	st, rec, err := prepareManagedRunStateWith(&cfg, runAdmissionDeps{
		openStore: func() (*state.Store, error) {
			openCalls++
			return nil, cause
		},
		newID: func() (string, error) {
			t.Fatal("newID called after state open failure")
			return "", nil
		},
		now: time.Now,
	})
	if !errors.Is(err, cause) {
		t.Fatalf("error=%v, want open cause", err)
	}
	if openCalls != 1 || st != nil || rec != nil {
		t.Fatalf("openCalls=%d store=%v rec=%v, want 1/nil/nil", openCalls, st, rec)
	}
	if cfg.ContainerID != "" {
		t.Fatalf("ContainerID=%q after failed open, want empty", cfg.ContainerID)
	}
}

func TestPrepareManagedRunStateFailsClosedWhenIDGenerationFails(t *testing.T) {
	cause := errors.New("entropy unavailable")
	cfg := runAdmissionTestConfig(t)
	opened, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}

	st, rec, err := prepareManagedRunStateWith(&cfg, runAdmissionDeps{
		openStore: func() (*state.Store, error) { return opened, nil },
		newID:     func() (string, error) { return "", cause },
		now:       time.Now,
	})
	if !errors.Is(err, cause) {
		t.Fatalf("error=%v, want ID cause", err)
	}
	if st != nil || rec != nil || cfg.ContainerID != "" {
		t.Fatalf("store=%v rec=%v ContainerID=%q after failed ID generation", st, rec, cfg.ContainerID)
	}
	if _, err := opened.List(); !errors.Is(err, state.ErrStoreClosed) {
		t.Fatalf("failed admission did not close store: %v", err)
	}
}

func TestPrepareManagedRunStateFailsClosedWhenCreatedStateSaveFails(t *testing.T) {
	cfg := runAdmissionTestConfig(t)
	closed, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := closed.Close(); err != nil {
		t.Fatal(err)
	}

	st, rec, err := prepareManagedRunStateWith(&cfg, runAdmissionDeps{
		openStore: func() (*state.Store, error) { return closed, nil },
		newID:     func() (string, error) { return runAdmissionTestID, nil },
		now:       time.Now,
	})
	if !errors.Is(err, state.ErrStoreClosed) {
		t.Fatalf("error=%v, want closed-store save failure", err)
	}
	if st != nil || rec != nil || cfg.ContainerID != "" {
		t.Fatalf("store=%v rec=%v ContainerID=%q after failed Save", st, rec, cfg.ContainerID)
	}
}

func TestPrepareManagedRunStatePublishesIDOnlyAfterDurableSave(t *testing.T) {
	cfg := runAdmissionTestConfig(t)
	createdAt := time.Unix(1_700_000_000, 123)
	opened, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()

	st, rec, err := prepareManagedRunStateWith(&cfg, runAdmissionDeps{
		openStore: func() (*state.Store, error) { return opened, nil },
		newID:     func() (string, error) { return runAdmissionTestID, nil },
		now:       func() time.Time { return createdAt },
	})
	if err != nil {
		t.Fatalf("prepareManagedRunStateWith: %v", err)
	}
	if st != opened || rec == nil {
		t.Fatalf("store=%v rec=%v, want opened store and record", st, rec)
	}
	if cfg.ContainerID != runAdmissionTestID || rec.ID != runAdmissionTestID {
		t.Fatalf("cfg ID=%q rec ID=%q, want %q", cfg.ContainerID, rec.ID, runAdmissionTestID)
	}
	if rec.Revision != 1 || rec.Status != state.StatusCreated || !rec.CreatedAt.Equal(createdAt) {
		t.Fatalf("record=%+v, want durable revision-1 created state", rec)
	}

	persisted, err := opened.Get(runAdmissionTestID)
	if err != nil {
		t.Fatalf("Get persisted record: %v", err)
	}
	if persisted.Revision != 1 || persisted.Status != state.StatusCreated || persisted.RootFS != cfg.RootFS || persisted.Hostname != cfg.Hostname {
		t.Fatalf("persisted=%+v, want admitted config", persisted)
	}
	if len(persisted.Command) != len(cfg.Command) || persisted.Command[0] != cfg.Command[0] {
		t.Fatalf("persisted command=%v, want %v", persisted.Command, cfg.Command)
	}
}

func TestPrepareManagedRunStateRejectsPreassignedContainerID(t *testing.T) {
	cfg := runAdmissionTestConfig(t)
	cfg.ContainerID = "already-assigned"
	opened := false

	st, rec, err := prepareManagedRunStateWith(&cfg, runAdmissionDeps{
		openStore: func() (*state.Store, error) {
			opened = true
			return nil, nil
		},
		newID: func() (string, error) { return runAdmissionTestID, nil },
		now:   time.Now,
	})
	if err == nil {
		t.Fatal("preassigned ContainerID was accepted")
	}
	if opened || st != nil || rec != nil || cfg.ContainerID != "already-assigned" {
		t.Fatalf("opened=%v store=%v rec=%v ContainerID=%q", opened, st, rec, cfg.ContainerID)
	}
}

func TestPrepareManagedRunStateRejectsMissingRootFSBeforeStateMutation(t *testing.T) {
	cfg := runAdmissionTestConfig(t)
	cfg.RootFS = filepath.Join(t.TempDir(), "missing")
	opened := false
	generated := false

	st, rec, err := prepareManagedRunStateWith(&cfg, runAdmissionDeps{
		openStore: func() (*state.Store, error) {
			opened = true
			return nil, nil
		},
		newID: func() (string, error) {
			generated = true
			return runAdmissionTestID, nil
		},
		now: time.Now,
	})
	if err == nil || !strings.Contains(err.Error(), "stat run rootfs") {
		t.Fatalf("error=%v, want rootfs stat failure", err)
	}
	if opened || generated || st != nil || rec != nil || cfg.ContainerID != "" {
		t.Fatalf("opened=%v generated=%v store=%v rec=%v ContainerID=%q", opened, generated, st, rec, cfg.ContainerID)
	}
}

func TestPrepareManagedRunStateRejectsNonDirectoryRootFSBeforeStateMutation(t *testing.T) {
	rootfs := filepath.Join(t.TempDir(), "rootfs-file")
	if err := os.WriteFile(rootfs, []byte("not a directory"), 0o600); err != nil {
		t.Fatal(err)
	}
	cfg := runAdmissionTestConfig(t)
	cfg.RootFS = rootfs
	opened := false

	st, rec, err := prepareManagedRunStateWith(&cfg, runAdmissionDeps{
		openStore: func() (*state.Store, error) {
			opened = true
			return nil, nil
		},
		newID: func() (string, error) { return runAdmissionTestID, nil },
		now:   time.Now,
	})
	if err == nil || !strings.Contains(err.Error(), "is not a directory") {
		t.Fatalf("error=%v, want non-directory rootfs rejection", err)
	}
	if opened || st != nil || rec != nil || cfg.ContainerID != "" {
		t.Fatalf("opened=%v store=%v rec=%v ContainerID=%q", opened, st, rec, cfg.ContainerID)
	}
}

func TestPrepareManagedRunStateCanonicalizesRelativeRootFSAtCommit(t *testing.T) {
	rootfs := t.TempDir()
	cwd, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	rel, err := filepath.Rel(cwd, rootfs)
	if err != nil {
		t.Fatal(err)
	}
	original := filepath.Join(rel, "ghost", "..")
	cfg := runAdmissionTestConfig(t)
	cfg.RootFS = original
	opened, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()

	st, rec, err := prepareManagedRunStateWith(&cfg, runAdmissionDeps{
		openStore: func() (*state.Store, error) { return opened, nil },
		newID:     func() (string, error) { return runAdmissionTestID, nil },
		now:       time.Now,
	})
	if err != nil {
		t.Fatalf("prepareManagedRunStateWith relative rootfs: %v", err)
	}
	expected := filepath.Clean(rootfs)
	if st != opened || rec == nil || cfg.RootFS != expected || rec.RootFS != expected {
		t.Fatalf("store=%v cfg.RootFS=%q rec=%+v, want canonical rootfs %q", st, cfg.RootFS, rec, expected)
	}
	if !filepath.IsAbs(cfg.RootFS) {
		t.Fatalf("committed rootfs %q is not absolute", cfg.RootFS)
	}
	persisted, err := opened.Get(runAdmissionTestID)
	if err != nil {
		t.Fatalf("Get persisted record: %v", err)
	}
	if persisted.RootFS != expected {
		t.Fatalf("persisted rootfs=%q, want %q", persisted.RootFS, expected)
	}
}

func TestPrepareManagedRunStateDoesNotPublishCanonicalRootFSOnFailure(t *testing.T) {
	rootfs := t.TempDir()
	cwd, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	rel, err := filepath.Rel(cwd, rootfs)
	if err != nil {
		t.Fatal(err)
	}
	cfg := runAdmissionTestConfig(t)
	cfg.RootFS = rel
	original := cfg.RootFS
	cause := errors.New("state unavailable")

	st, rec, err := prepareManagedRunStateWith(&cfg, runAdmissionDeps{
		openStore: func() (*state.Store, error) { return nil, cause },
		newID:     func() (string, error) { return runAdmissionTestID, nil },
		now:       time.Now,
	})
	if !errors.Is(err, cause) {
		t.Fatalf("error=%v, want open cause", err)
	}
	if st != nil || rec != nil || cfg.RootFS != original || cfg.ContainerID != "" {
		t.Fatalf("store=%v rec=%v RootFS=%q ContainerID=%q, want config unchanged", st, rec, cfg.RootFS, cfg.ContainerID)
	}
}

func TestPrepareManagedRunStateResolvesRootFSSymlinkToDirectory(t *testing.T) {
	target := t.TempDir()
	link := filepath.Join(t.TempDir(), "rootfs-link")
	if err := os.Symlink(target, link); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	cfg := runAdmissionTestConfig(t)
	cfg.RootFS = link
	opened, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()

	st, rec, err := prepareManagedRunStateWith(&cfg, runAdmissionDeps{
		openStore: func() (*state.Store, error) { return opened, nil },
		newID:     func() (string, error) { return runAdmissionTestID, nil },
		now:       time.Now,
	})
	if err != nil {
		t.Fatalf("prepareManagedRunStateWith symlink rootfs: %v", err)
	}
	expected, err := filepath.EvalSymlinks(target)
	if err != nil {
		t.Fatal(err)
	}
	expected = filepath.Clean(expected)
	if st != opened || rec == nil || rec.RootFS != expected || cfg.RootFS != expected || cfg.ContainerID != runAdmissionTestID {
		t.Fatalf("store=%v rec=%+v RootFS=%q ContainerID=%q, want resolved rootfs %q", st, rec, cfg.RootFS, cfg.ContainerID, expected)
	}
}

func TestPrepareManagedRunStatePinsRootFSSymlinkTargetAcrossRetarget(t *testing.T) {
	first := t.TempDir()
	second := t.TempDir()
	link := filepath.Join(t.TempDir(), "rootfs-link")
	if err := os.Symlink(first, link); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	cfg := runAdmissionTestConfig(t)
	cfg.RootFS = link
	opened, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()

	_, rec, err := prepareManagedRunStateWith(&cfg, runAdmissionDeps{
		openStore: func() (*state.Store, error) { return opened, nil },
		newID:     func() (string, error) { return runAdmissionTestID, nil },
		now:       time.Now,
	})
	if err != nil {
		t.Fatalf("prepareManagedRunStateWith symlink rootfs: %v", err)
	}
	firstResolved, err := filepath.EvalSymlinks(first)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(link); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(second, link); err != nil {
		t.Fatal(err)
	}
	secondResolved, err := filepath.EvalSymlinks(link)
	if err != nil {
		t.Fatal(err)
	}
	if secondResolved == firstResolved {
		t.Fatalf("retargeted symlink still resolves to first target %q", firstResolved)
	}
	if cfg.RootFS != firstResolved || rec.RootFS != firstResolved {
		t.Fatalf("after retarget cfg.RootFS=%q rec.RootFS=%q, want pinned first target %q", cfg.RootFS, rec.RootFS, firstResolved)
	}
	persisted, err := opened.Get(runAdmissionTestID)
	if err != nil {
		t.Fatalf("Get persisted record: %v", err)
	}
	if persisted.RootFS != firstResolved {
		t.Fatalf("persisted rootfs=%q after symlink retarget, want pinned first target %q", persisted.RootFS, firstResolved)
	}
}

func runAdmissionTestConfig(t *testing.T) container.Config {
	t.Helper()
	return container.Config{
		RootFS:   t.TempDir(),
		Command:  []string{"/bin/true"},
		Hostname: "minicontainer",
	}
}
