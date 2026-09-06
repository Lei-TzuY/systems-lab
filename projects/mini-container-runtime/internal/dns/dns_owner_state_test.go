//go:build linux

package dns

import (
	"os/exec"
	"testing"

	"minicontainer/internal/state"
)

func TestHostEntryOwnerActiveBoundGenerationIgnoresLiveRegistrarAfterChildDeath(t *testing.T) {
	owner, err := currentRegistrarIdentity()
	if err != nil {
		t.Fatalf("current registrar identity: %v", err)
	}

	child := exec.Command("sleep", "30")
	if err := child.Start(); err != nil {
		t.Fatalf("start child fixture: %v", err)
	}
	childStart, err := readProcessStartTime(child.Process.Pid)
	if err != nil {
		_ = child.Process.Kill()
		_ = child.Wait()
		t.Fatalf("child start time: %v", err)
	}
	if err := child.Process.Kill(); err != nil {
		_ = child.Wait()
		t.Fatalf("kill child fixture: %v", err)
	}
	if err := child.Wait(); err == nil {
		t.Fatal("killed child fixture unexpectedly exited successfully")
	}

	active, err := hostEntryOwnerActive(HostEntry{
		ContainerID:         "dead-child",
		Hostname:            "dead-child",
		IP:                  "172.20.0.2",
		OwnerPID:            owner.PID,
		OwnerStartTime:      owner.StartTime,
		GenerationAware:     true,
		GenerationPID:       child.Process.Pid,
		GenerationStartTime: childStart,
	})
	if err != nil {
		t.Fatalf("probe bound dead child: %v", err)
	}
	if active {
		t.Fatal("bound DNS entry remained active solely because registrar was alive")
	}
}

func TestHostEntryOwnerActiveBoundGenerationSurvivesWithoutRegistrarOrState(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)

	child := exec.Command("sleep", "30")
	if err := child.Start(); err != nil {
		t.Fatalf("start child fixture: %v", err)
	}
	defer func() {
		_ = child.Process.Kill()
		_ = child.Wait()
	}()
	childStart, err := readProcessStartTime(child.Process.Pid)
	if err != nil {
		t.Fatalf("child start time: %v", err)
	}

	active, err := hostEntryOwnerActive(HostEntry{
		ContainerID:         "live-child",
		Hostname:            "live-child",
		IP:                  "172.20.0.2",
		OwnerPID:            2147483647,
		OwnerStartTime:      1,
		GenerationAware:     true,
		GenerationPID:       child.Process.Pid,
		GenerationStartTime: childStart,
	})
	if err != nil {
		t.Fatalf("probe bound live child: %v", err)
	}
	if !active {
		t.Fatal("bound live child was not authoritative")
	}
}

func TestHostEntryOwnerActiveRejectsIncompleteBoundGeneration(t *testing.T) {
	_, err := hostEntryOwnerActive(HostEntry{
		ContainerID:     "partial-generation",
		GenerationAware: true,
		GenerationPID:   1234,
	})
	if err == nil {
		t.Fatal("incomplete child generation identity was accepted")
	}
}

func TestOwnerlessLegacyEntryWithoutContainerStateIsInactive(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)

	active, err := hostEntryOwnerActive(HostEntry{
		ContainerID: "deleted-legacy",
		Hostname:    "old-web",
		IP:          "172.20.0.9",
	})
	if err != nil {
		t.Fatalf("probe ownerless orphan: %v", err)
	}
	if active {
		t.Fatal("ownerless DNS entry without durable container state remained authoritative")
	}
}

func TestOwnerlessLegacyEntryPreservesMatchingLiveContainer(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)

	child := exec.Command("sleep", "30")
	if err := child.Start(); err != nil {
		t.Fatalf("start child fixture: %v", err)
	}
	defer func() {
		_ = child.Process.Kill()
		_ = child.Wait()
	}()
	childStart, err := readProcessStartTime(child.Process.Pid)
	if err != nil {
		t.Fatalf("child start time: %v", err)
	}

	st, err := state.Open(state.DefaultDir())
	if err != nil {
		t.Fatalf("open state: %v", err)
	}
	if err := st.Save(&state.Container{
		ID:           "legacy-live",
		PID:          child.Process.Pid,
		PIDStartTime: childStart,
		Status:       state.StatusRunning,
		Hostname:     "legacy-web",
	}); err != nil {
		_ = st.Close()
		t.Fatalf("save running state: %v", err)
	}
	if err := st.Close(); err != nil {
		t.Fatalf("close state: %v", err)
	}

	active, err := hostEntryOwnerActive(HostEntry{
		ContainerID: "legacy-live",
		Hostname:    "legacy-web",
		IP:          "172.20.0.8",
	})
	if err != nil {
		t.Fatalf("probe ownerless live entry: %v", err)
	}
	if !active {
		t.Fatal("ownerless DNS entry for a matching live container was pruned")
	}
}

func TestRegisterHostReclaimsOrphanedOwnerlessLegacyHostname(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)

	const networkName = "legacy-migration"
	dir, err := ensureDNSDir()
	if err != nil {
		t.Fatalf("ensure DNS dir: %v", err)
	}
	netFile := dir + "/" + networkName + ".json"
	if err := saveEntriesAtomic(dir, netFile, networkName, []HostEntry{{
		ContainerID: "deleted-legacy",
		Hostname:    "web",
		IP:          "172.20.0.8",
	}}); err != nil {
		t.Fatalf("seed ownerless legacy entry: %v", err)
	}

	if err := RegisterHost(networkName, "replacement", "web", "172.20.0.9"); err != nil {
		t.Fatalf("replacement registration stayed blocked by orphaned legacy entry: %v", err)
	}

	entries, exists, err := loadEntriesChecked(netFile, networkName)
	if err != nil {
		t.Fatalf("load migrated registry: %v", err)
	}
	if !exists || len(entries) != 1 {
		t.Fatalf("migrated registry entries = %#v, exists=%v", entries, exists)
	}
	if entries[0].ContainerID != "replacement" || entries[0].Hostname != "web" || !entries[0].GenerationAware {
		t.Fatalf("unexpected replacement entry: %#v", entries[0])
	}
}
