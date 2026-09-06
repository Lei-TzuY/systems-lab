//go:build linux

package dns

import (
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

const dnsCrashHelperEnv = "MINICONTAINER_DNS_CRASH_HELPER"

func TestDNSRegistrarCrashHelper(t *testing.T) {
	if os.Getenv(dnsCrashHelperEnv) != "1" {
		return
	}
	if err := RegisterHost("default", "crashed-container", "crashed-host", "10.0.0.9"); err != nil {
		os.Exit(91)
	}
	// Deliberately bypass all Go defers, matching cmdRun's os.Exit failure path.
	os.Exit(17)
}

func TestDNSRegistryPrunesRegistrationAfterRegistrarOsExit(t *testing.T) {
	home := t.TempDir()
	cmd := exec.Command(os.Args[0], "-test.run=^TestDNSRegistrarCrashHelper$")
	cmd.Env = append(os.Environ(),
		dnsCrashHelperEnv+"=1",
		"HOME="+home,
		"USERPROFILE="+home,
	)
	err := cmd.Run()
	var exitErr *exec.ExitError
	if !errors.As(err, &exitErr) || exitErr.ExitCode() != 17 {
		t.Fatalf("helper exit=%v, want status 17", err)
	}

	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)
	content, err := GenerateHostsContentChecked("default")
	if err != nil {
		t.Fatalf("generate after registrar exit: %v", err)
	}
	if strings.Contains(content, "crashed-host") || strings.Contains(content, "10.0.0.9") {
		t.Fatalf("stale crashed registration remained authoritative:\n%s", content)
	}

	entries, exists, err := loadEntriesChecked(filepath.Join(DefaultDNSDir(), "default.json"), "default")
	if err != nil || !exists {
		t.Fatalf("load pruned registry: entries=%+v exists=%v err=%v", entries, exists, err)
	}
	if len(entries) != 0 {
		t.Fatalf("stale registration remained on disk: %+v", entries)
	}
}

func TestRegistrarGenerationAliveRejectsUnreapedZombie(t *testing.T) {
	cmd := exec.Command("sh", "-c", "exit 0")
	if err := cmd.Start(); err != nil {
		t.Fatal(err)
	}
	defer func() { _ = cmd.Wait() }()

	pid := cmd.Process.Pid
	start, err := readProcessStartTime(pid)
	if err != nil {
		t.Fatalf("read child start time: %v", err)
	}

	deadline := time.Now().Add(2 * time.Second)
	for {
		observedStart, processState, err := readProcessStat(pid)
		if err != nil {
			t.Fatalf("read child process state: %v", err)
		}
		if observedStart != start {
			t.Fatalf("child generation changed while awaiting zombie: start=%d observed=%d", start, observedStart)
		}
		if processState == "Z" {
			break
		}
		if time.Now().After(deadline) {
			t.Fatalf("child did not become zombie; last state=%q", processState)
		}
		time.Sleep(10 * time.Millisecond)
	}

	alive, err := registrarGenerationAlive(pid, start)
	if err != nil {
		t.Fatalf("probe zombie generation: %v", err)
	}
	if alive {
		t.Fatalf("unreaped zombie %d/%d remained authoritative", pid, start)
	}
}

func TestRegisterHostPersistsCurrentRegistrarGeneration(t *testing.T) {
	useTempDNSHome(t)
	if err := RegisterHost("default", "live-container", "live-host", "10.0.0.2"); err != nil {
		t.Fatal(err)
	}
	entries, ok, err := loadEntriesChecked(filepath.Join(DefaultDNSDir(), "default.json"), "default")
	if err != nil || !ok || len(entries) != 1 {
		t.Fatalf("entries=%+v ok=%v err=%v", entries, ok, err)
	}
	owner, err := currentRegistrarIdentity()
	if err != nil {
		t.Fatal(err)
	}
	if entries[0].OwnerPID != owner.PID || entries[0].OwnerStartTime != owner.StartTime {
		t.Fatalf("owner=%d/%d, want %d/%d", entries[0].OwnerPID, entries[0].OwnerStartTime, owner.PID, owner.StartTime)
	}
	content, err := GenerateHostsContentChecked("default")
	if err != nil || !strings.Contains(content, "10.0.0.2\tlive-host") {
		t.Fatalf("live registration lost: content=%q err=%v", content, err)
	}
}

func TestDNSRegistryPrunesPIDReuseMismatch(t *testing.T) {
	useTempDNSHome(t)
	owner, err := currentRegistrarIdentity()
	if err != nil {
		t.Fatal(err)
	}
	entry := HostEntry{
		ContainerID:    "stale-container",
		Hostname:       "stale-host",
		IP:             "10.0.0.4",
		OwnerPID:       owner.PID,
		OwnerStartTime: owner.StartTime + 1,
	}
	dir, err := ensureDNSDir()
	if err != nil {
		t.Fatal(err)
	}
	if err := saveEntriesAtomic(dir, filepath.Join(dir, "default.json"), "default", []HostEntry{entry}); err != nil {
		t.Fatal(err)
	}
	content, err := GenerateHostsContentChecked("default")
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(content, "stale-host") {
		t.Fatalf("PID-reused registration survived:\n%s", content)
	}
}

func TestDNSRegistryPrunesLegacyUnownedEntryWithoutState(t *testing.T) {
	useTempDNSHome(t)
	dir, err := ensureDNSDir()
	if err != nil {
		t.Fatal(err)
	}
	legacy := HostEntry{ContainerID: "legacy", Hostname: "legacy-host", IP: "10.0.0.5"}
	path := filepath.Join(dir, "default.json")
	if err := saveEntriesAtomic(dir, path, "default", []HostEntry{legacy}); err != nil {
		t.Fatal(err)
	}
	content, err := GenerateHostsContentChecked("default")
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(content, "10.0.0.5\tlegacy-host") {
		t.Fatalf("orphaned ownerless legacy entry remained authoritative: %q", content)
	}
	entries, ok, err := loadEntriesChecked(path, "default")
	if err != nil || !ok {
		t.Fatalf("load pruned registry: entries=%+v ok=%v err=%v", entries, ok, err)
	}
	if len(entries) != 0 {
		t.Fatalf("orphaned ownerless legacy entry remained on disk: %+v", entries)
	}
}

func TestDNSRegistryRejectsPartialRegistrarIdentity(t *testing.T) {
	useTempDNSHome(t)
	dir, err := ensureDNSDir()
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(dir, "default.json")
	const malformed = `{"schema_version":1,"network_name":"default","entries":[{"schema_version":1,"container_id":"bad","hostname":"bad-host","ip":"10.0.0.6","owner_pid":123}]}`
	if err := os.WriteFile(path, []byte(malformed), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := GenerateHostsContentChecked("default"); err == nil || !strings.Contains(err.Error(), "invalid ownership") {
		t.Fatalf("partial registrar identity error=%v", err)
	}
	got, err := os.ReadFile(path)
	if err != nil || string(got) != malformed {
		t.Fatalf("malformed registry was mutated: data=%q err=%v", got, err)
	}
}
