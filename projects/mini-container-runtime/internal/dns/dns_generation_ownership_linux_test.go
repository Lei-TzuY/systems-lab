//go:build linux

package dns

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func readSingleHostEntry(t *testing.T, networkName string) HostEntry {
	t.Helper()
	entries, exists, err := loadEntriesChecked(filepath.Join(DefaultDNSDir(), networkName+".json"), networkName)
	if err != nil {
		t.Fatal(err)
	}
	if !exists || len(entries) != 1 {
		t.Fatalf("registry exists=%v entries=%d, want true/1", exists, len(entries))
	}
	return entries[0]
}

func TestRegisterHostPublishesUnboundGenerationAwareEntry(t *testing.T) {
	useTempDNSHome(t)
	if err := RegisterHost("default", "ctr-modern-unbound", "modern-host", "10.0.0.2"); err != nil {
		t.Fatal(err)
	}
	entry := readSingleHostEntry(t, "default")
	if !entry.GenerationAware {
		t.Fatal("modern registration was not marked generation-aware")
	}
	if entry.GenerationPID != 0 || entry.GenerationStartTime != 0 {
		t.Fatalf("pre-admission registration unexpectedly bound to %d/%d", entry.GenerationPID, entry.GenerationStartTime)
	}
}

func TestBindHostRegistrationGenerationPersistsExactIdentityAndIsIdempotent(t *testing.T) {
	useTempDNSHome(t)
	if err := RegisterHost("default", "ctr-bind", "bind-host", "10.0.0.3"); err != nil {
		t.Fatal(err)
	}
	if err := BindHostRegistrationGeneration("default", "ctr-bind", 4242, 99); err != nil {
		t.Fatal(err)
	}
	if err := BindHostRegistrationGeneration("default", "ctr-bind", 4242, 99); err != nil {
		t.Fatalf("idempotent bind failed: %v", err)
	}
	entry := readSingleHostEntry(t, "default")
	if !entry.GenerationAware || entry.GenerationPID != 4242 || entry.GenerationStartTime != 99 {
		t.Fatalf("wrong durable generation binding: %+v", entry)
	}
	if err := BindHostRegistrationGeneration("default", "ctr-bind", 4343, 100); err == nil {
		t.Fatal("conflicting child generation rebound existing registration")
	}
}

func TestRegisterHostRefreshClearsPriorChildBinding(t *testing.T) {
	useTempDNSHome(t)
	if err := RegisterHost("default", "ctr-refresh", "refresh-host", "10.0.0.4"); err != nil {
		t.Fatal(err)
	}
	if err := BindHostRegistrationGeneration("default", "ctr-refresh", 5151, 111); err != nil {
		t.Fatal(err)
	}
	if err := RegisterHost("default", "ctr-refresh", "refresh-host", "10.0.0.4"); err != nil {
		t.Fatal(err)
	}
	entry := readSingleHostEntry(t, "default")
	if !entry.GenerationAware || entry.GenerationPID != 0 || entry.GenerationStartTime != 0 {
		t.Fatalf("new attempt retained stale child binding: %+v", entry)
	}
}

func TestDeadRegistrarModernUnboundEntryCannotUseNetworkReservationAsAdoptionProof(t *testing.T) {
	useTempDNSHome(t)
	identity, err := currentRegistrarIdentity()
	if err != nil {
		t.Fatal(err)
	}
	const containerID = "modern-pre-bind-crash"
	st, err := state.Open(state.DefaultDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&state.Container{
		ID:        containerID,
		Status:    state.StatusCreated,
		RootFS:    "/tmp/rootfs",
		Command:   []string{"true"},
		Hostname:  "modern-pre-bind",
		CreatedAt: time.Now(),
	}); err != nil {
		_ = st.Close()
		t.Fatal(err)
	}
	if err := st.MarkRunning(containerID, os.Getpid(), identity.StartTime, time.Now()); err != nil {
		_ = st.Close()
		t.Fatal(err)
	}
	markDNSBridgeOwnership(t, st, containerID, os.Getpid(), identity.StartTime)
	if err := st.Close(); err != nil {
		t.Fatal(err)
	}

	saveDeadRegistrarEntry(t, HostEntry{
		ContainerID:     containerID,
		Hostname:        "modern-pre-bind",
		IP:              "10.0.0.5",
		OwnerPID:        99999999,
		OwnerStartTime:  1,
		GenerationAware: true,
	})
	content, err := GenerateHostsContentChecked("default")
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(content, "modern-pre-bind") {
		t.Fatalf("unbound modern registration survived registrar crash:\n%s", content)
	}
}

func TestDeadRegistrarModernEntryRejectsDifferentRunningGeneration(t *testing.T) {
	useTempDNSHome(t)
	identity, err := currentRegistrarIdentity()
	if err != nil {
		t.Fatal(err)
	}
	const containerID = "modern-generation-mismatch"
	st, err := state.Open(state.DefaultDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&state.Container{
		ID:        containerID,
		Status:    state.StatusCreated,
		RootFS:    "/tmp/rootfs",
		Command:   []string{"true"},
		Hostname:  "modern-mismatch",
		CreatedAt: time.Now(),
	}); err != nil {
		_ = st.Close()
		t.Fatal(err)
	}
	if err := st.MarkRunning(containerID, os.Getpid(), identity.StartTime, time.Now()); err != nil {
		_ = st.Close()
		t.Fatal(err)
	}
	markDNSBridgeOwnership(t, st, containerID, os.Getpid(), identity.StartTime)
	if err := st.Close(); err != nil {
		t.Fatal(err)
	}

	saveDeadRegistrarEntry(t, HostEntry{
		ContainerID:         containerID,
		Hostname:            "modern-mismatch",
		IP:                  "10.0.0.6",
		OwnerPID:            99999999,
		OwnerStartTime:      1,
		GenerationAware:     true,
		GenerationPID:       os.Getpid(),
		GenerationStartTime: identity.StartTime + 1,
	})
	content, err := GenerateHostsContentChecked("default")
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(content, "modern-mismatch") {
		t.Fatalf("different running generation adopted exact-owned registration:\n%s", content)
	}
}
