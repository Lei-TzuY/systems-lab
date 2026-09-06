//go:build linux

package dns

import "testing"

func TestCleanupStoppedGenerationConsumesExactModernRegistration(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)

	owner := registrarIdentity{PID: 101, StartTime: 1001}
	entry := HostEntry{
		ContainerID:         "target",
		Hostname:            "target",
		IP:                  "172.20.0.2",
		OwnerPID:            owner.PID,
		OwnerStartTime:      owner.StartTime,
		GenerationAware:     true,
		GenerationPID:       202,
		GenerationStartTime: 2002,
	}
	writeOwnedDNSRegistry(t, "default", []HostEntry{entry})

	err := cleanupStoppedHostRegistrationWithGenerationPolicy(
		"default",
		"target",
		childGenerationIdentity{PID: 202, StartTime: 2002},
	)
	if err != nil {
		t.Fatalf("cleanup exact modern generation: %v", err)
	}
	if entries := readOwnedDNSRegistry(t, "default"); len(entries) != 0 {
		t.Fatalf("exact generation registration remains: %+v", entries)
	}
}

func TestCleanupStoppedGenerationRetiresOwnerlessLegacyInSamePolicy(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)

	ownerless := HostEntry{
		ContainerID: "target",
		Hostname:    "old-target",
		IP:          "172.20.0.2",
	}
	other := HostEntry{
		ContainerID:         "other",
		Hostname:            "other",
		IP:                  "172.20.0.8",
		OwnerPID:            808,
		OwnerStartTime:      8008,
		GenerationAware:     true,
		GenerationPID:       909,
		GenerationStartTime: 9009,
	}
	writeOwnedDNSRegistry(t, "default", []HostEntry{ownerless, other})

	err := cleanupStoppedHostRegistrationWithGenerationPolicy(
		"default",
		"target",
		childGenerationIdentity{PID: 202, StartTime: 2002},
	)
	if err != nil {
		t.Fatalf("cleanup ownerless legacy under exact generation authority: %v", err)
	}
	entries := readOwnedDNSRegistry(t, "default")
	if len(entries) != 1 || entries[0] != other {
		t.Fatalf("ownerless migration cleanup changed unrelated modern entry: %+v", entries)
	}
}

func TestCleanupStoppedGenerationPreservesNewerSameRegistrarGeneration(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)

	owner := registrarIdentity{PID: 303, StartTime: 3003}
	replacement := HostEntry{
		ContainerID:         "target",
		Hostname:            "target",
		IP:                  "172.20.0.3",
		OwnerPID:            owner.PID,
		OwnerStartTime:      owner.StartTime,
		GenerationAware:     true,
		GenerationPID:       404,
		GenerationStartTime: 4004,
	}
	writeOwnedDNSRegistry(t, "default", []HostEntry{replacement})

	err := cleanupStoppedHostRegistrationWithGenerationPolicy(
		"default",
		"target",
		childGenerationIdentity{PID: 202, StartTime: 2002},
	)
	if err != nil {
		t.Fatalf("cleanup stale finalizer against replacement: %v", err)
	}
	entries := readOwnedDNSRegistry(t, "default")
	if len(entries) != 1 || entries[0] != replacement {
		t.Fatalf("newer same-registrar generation changed: %+v", entries)
	}
}

func TestCleanupStoppedGenerationPreservesUnboundNewAttempt(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)

	owner := registrarIdentity{PID: 505, StartTime: 5005}
	unbound := HostEntry{
		ContainerID:     "target",
		Hostname:        "target",
		IP:              "172.20.0.4",
		OwnerPID:        owner.PID,
		OwnerStartTime:  owner.StartTime,
		GenerationAware: true,
	}
	writeOwnedDNSRegistry(t, "default", []HostEntry{unbound})

	err := cleanupStoppedHostRegistrationWithGenerationPolicy(
		"default",
		"target",
		childGenerationIdentity{PID: 202, StartTime: 2002},
	)
	if err != nil {
		t.Fatalf("cleanup stale finalizer against unbound attempt: %v", err)
	}
	entries := readOwnedDNSRegistry(t, "default")
	if len(entries) != 1 || entries[0] != unbound {
		t.Fatalf("unbound newer attempt changed: %+v", entries)
	}
}

func TestCleanupStoppedGenerationUsesChildIdentityAcrossRegistrarDeath(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)

	foreign := HostEntry{
		ContainerID:         "target",
		Hostname:            "target",
		IP:                  "172.20.0.2",
		OwnerPID:            606,
		OwnerStartTime:      6006,
		GenerationAware:     true,
		GenerationPID:       707,
		GenerationStartTime: 7007,
	}
	writeOwnedDNSRegistry(t, "default", []HostEntry{foreign})

	err := cleanupStoppedHostRegistrationWithGenerationPolicy(
		"default",
		"target",
		childGenerationIdentity{PID: 707, StartTime: 7007},
	)
	if err != nil {
		t.Fatalf("cleanup exact child generation across registrar death: %v", err)
	}
	if entries := readOwnedDNSRegistry(t, "default"); len(entries) != 0 {
		t.Fatalf("exact foreign-registrar generation remains: %+v", entries)
	}
}

func TestCleanupStoppedGenerationRejectsIncompleteIdentity(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)

	err := cleanupStoppedHostRegistrationWithGenerationPolicy(
		"default",
		"target",
		childGenerationIdentity{PID: 0, StartTime: 1},
	)
	if err == nil {
		t.Fatal("incomplete child generation unexpectedly accepted")
	}
}
