//go:build linux

package network

import (
	"errors"
	"net"
	"testing"
)

func TestRemoveVethHostReportsInterfaceEnumerationFailure(t *testing.T) {
	cause := errors.New("route socket failed")
	deleteCalls := 0
	err := removeVethHostWith(42, false, func() ([]net.Interface, error) {
		return nil, cause
	}, func(string, int) error {
		deleteCalls++
		return nil
	})
	if !errors.Is(err, cause) {
		t.Fatalf("enumeration failure not preserved: %v", err)
	}
	if deleteCalls != 0 {
		t.Fatalf("delete called %d times after lookup failure", deleteCalls)
	}
}

func TestRemoveVethHostTreatsConfirmedAbsenceAsClean(t *testing.T) {
	deleteCalls := 0
	err := removeVethHostWith(43, false, func() ([]net.Interface, error) {
		return []net.Interface{{Index: 1, Name: "lo"}}, nil
	}, func(string, int) error {
		deleteCalls++
		return nil
	})
	if err != nil {
		t.Fatalf("confirmed absence returned error: %v", err)
	}
	if deleteCalls != 0 {
		t.Fatalf("delete called %d times for absent veth", deleteCalls)
	}
}

func TestRemoveVethHostDeletesExactDiscoveredInterface(t *testing.T) {
	cause := errors.New("netlink delete failed")
	deleteCalls := 0
	err := removeVethHostWith(44, false, func() ([]net.Interface, error) {
		return []net.Interface{
			{Index: 1, Name: "lo"},
			{Index: 77, Name: VethHostIface(44)},
		}, nil
	}, func(name string, index int) error {
		deleteCalls++
		if name != VethHostIface(44) || index != 77 {
			t.Fatalf("delete target=%q/%d", name, index)
		}
		return cause
	})
	if !errors.Is(err, cause) {
		t.Fatalf("delete failure not preserved: %v", err)
	}
	if deleteCalls != 1 {
		t.Fatalf("delete calls=%d, want 1", deleteCalls)
	}
}

func TestRemoveVethHostRejectsNilOperations(t *testing.T) {
	if err := removeVethHostWith(1, false, nil, func(string, int) error { return nil }); err == nil {
		t.Fatal("nil interface lister unexpectedly accepted")
	}
	if err := removeVethHostWith(1, false, func() ([]net.Interface, error) { return nil, nil }, nil); err == nil {
		t.Fatal("nil link deleter unexpectedly accepted")
	}
}
