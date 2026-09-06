package dns

import (
	"reflect"
	"strings"
	"testing"
)

func TestEntriesWithRegistrationUpgradesLegacySameRegistrar(t *testing.T) {
	owner := registrarIdentity{PID: 101, StartTime: 1001}
	entries := []HostEntry{{ContainerID: "ctr-a", Hostname: "app-a", IP: "172.20.0.2", OwnerPID: owner.PID, OwnerStartTime: owner.StartTime}}
	got, changed, err := entriesWithRegistration(entries, owner, "ctr-a", "app-a", "172.20.0.2", false)
	if err != nil { t.Fatalf("same registrar legacy upgrade: %v", err) }
	if !changed { t.Fatal("legacy same-registrar registration was not upgraded") }
	if len(got) != 1 || !got[0].GenerationAware || got[0].GenerationPID != 0 || got[0].GenerationStartTime != 0 { t.Fatalf("legacy upgrade produced wrong entry: %+v", got) }
	if entries[0].GenerationAware { t.Fatal("legacy upgrade mutated caller input") }
}

func TestEntriesWithRegistrationIsIdempotentForModernUnboundSameRegistrar(t *testing.T) {
	owner := registrarIdentity{PID: 101, StartTime: 1001}
	entries := []HostEntry{{ContainerID: "ctr-a", Hostname: "app-a", IP: "172.20.0.2", OwnerPID: owner.PID, OwnerStartTime: owner.StartTime, GenerationAware: true}}
	got, changed, err := entriesWithRegistration(entries, owner, "ctr-a", "app-a", "172.20.0.2", false)
	if err != nil { t.Fatalf("same registrar registration: %v", err) }
	if changed { t.Fatal("modern unbound same-registrar registration unexpectedly reported mutation") }
	if !reflect.DeepEqual(got, entries) { t.Fatalf("same registrar changed entries: got=%+v want=%+v", got, entries) }
}

func TestEntriesWithRegistrationRejectsLiveForeignContainerOwner(t *testing.T) {
	foreign := HostEntry{ContainerID: "ctr-a", Hostname: "app-a", IP: "172.20.0.2", OwnerPID: 202, OwnerStartTime: 2002}
	entries := []HostEntry{foreign}
	got, changed, err := entriesWithRegistration(entries, registrarIdentity{PID: 303, StartTime: 3003}, "ctr-a", "app-a", "172.20.0.2", false)
	if err == nil || !strings.Contains(err.Error(), "live DNS registration conflict") { t.Fatalf("foreign container owner was accepted: got=%+v changed=%v err=%v", got, changed, err) }
	if changed || got != nil { t.Fatalf("foreign conflict exposed replacement state: got=%+v changed=%v", got, changed) }
	if !reflect.DeepEqual(entries, []HostEntry{foreign}) { t.Fatalf("foreign conflict mutated input: %+v", entries) }
}

func TestEntriesWithRegistrationRejectsLiveForeignHostnameOwner(t *testing.T) {
	entries := []HostEntry{{ContainerID: "ctr-newer", Hostname: "shared-name", IP: "172.20.0.9", OwnerPID: 404, OwnerStartTime: 4004}}
	_, changed, err := entriesWithRegistration(entries, registrarIdentity{PID: 505, StartTime: 5005}, "ctr-stale", "shared-name", "172.20.0.2", false)
	if err == nil { t.Fatal("foreign hostname owner was accepted") }
	if changed { t.Fatal("foreign hostname conflict reported mutation") }
}

func TestEntriesWithRegistrationAppendsDistinctRegistration(t *testing.T) {
	entries := []HostEntry{{ContainerID: "ctr-a", Hostname: "app-a", IP: "172.20.0.8", OwnerPID: 606, OwnerStartTime: 6006}}
	owner := registrarIdentity{PID: 707, StartTime: 7007}
	got, changed, err := entriesWithRegistration(entries, owner, "ctr-b", "app-b", "172.20.0.2", false)
	if err != nil { t.Fatalf("distinct registration: %v", err) }
	if !changed || len(got) != 2 { t.Fatalf("distinct registration got=%+v changed=%v", got, changed) }
	if got[1].ContainerID != "ctr-b" || got[1].Hostname != "app-b" || got[1].OwnerPID != owner.PID || got[1].OwnerStartTime != owner.StartTime || !got[1].GenerationAware { t.Fatalf("wrong appended registration: %+v", got[1]) }
}
