//go:build linux

package network

import (
	"errors"
	"net"
	"os"
	"testing"
)

func TestRemoveVethHostOwnedLeavesForeignSameNameInterfaceIntact(t *testing.T) {
	owner := "minicontainer:0123456789abcdef0123456789abcdef"
	name := VethHostIfaceOwned(owner)
	deleteCalls := 0
	listCalls := 0
	err := removeVethHostOwnedWith(name, owner, false, func() ([]net.Interface, error) {
		listCalls++
		return []net.Interface{{Index: 77, Name: name}}, nil
	}, func(got string) (string, error) {
		if got != name {
			t.Fatalf("alias target=%q, want %q", got, name)
		}
		return "minicontainer:foreign-owner", nil
	}, func(string, int) error {
		deleteCalls++
		return nil
	})
	if err != nil {
		t.Fatalf("foreign same-name cleanup returned error: %v", err)
	}
	if listCalls != 1 || deleteCalls != 0 {
		t.Fatalf("foreign cleanup listCalls=%d deleteCalls=%d", listCalls, deleteCalls)
	}
}

func TestRemoveVethHostOwnedDeletesExactMatchingGeneration(t *testing.T) {
	owner := "minicontainer:0123456789abcdef0123456789abcdef"
	name := VethHostIfaceOwned(owner)
	listCalls := 0
	aliasCalls := 0
	deleteCalls := 0
	err := removeVethHostOwnedWith(name, owner, false, func() ([]net.Interface, error) {
		listCalls++
		return []net.Interface{{Index: 88, Name: name}}, nil
	}, func(string) (string, error) {
		aliasCalls++
		return owner, nil
	}, func(got string, index int) error {
		deleteCalls++
		if got != name || index != 88 {
			t.Fatalf("delete target=%q/%d, want %q/88", got, index, name)
		}
		return nil
	})
	if err != nil {
		t.Fatalf("matching cleanup: %v", err)
	}
	if listCalls != 2 || aliasCalls != 2 || deleteCalls != 1 {
		t.Fatalf("matching cleanup listCalls=%d aliasCalls=%d deleteCalls=%d", listCalls, aliasCalls, deleteCalls)
	}
}

func TestRemoveVethHostOwnedRejectsNameNotDerivedFromOwner(t *testing.T) {
	owner := "minicontainer:0123456789abcdef0123456789abcdef"
	other := "minicontainer:fedcba9876543210fedcba9876543210"
	listCalls := 0
	err := removeVethHostOwnedWith(VethHostIfaceOwned(other), owner, false, func() ([]net.Interface, error) {
		listCalls++
		return nil, nil
	}, func(string) (string, error) {
		t.Fatal("alias read after identity mismatch")
		return "", nil
	}, func(string, int) error {
		t.Fatal("delete after identity mismatch")
		return nil
	})
	if err == nil {
		t.Fatal("veth name not derived from owner was accepted")
	}
	if listCalls != 0 {
		t.Fatalf("interface list calls=%d after identity mismatch", listCalls)
	}
}

func TestRemoveVethHostOwnedPropagatesAliasInspectionFailure(t *testing.T) {
	owner := "minicontainer:0123456789abcdef0123456789abcdef"
	name := VethHostIfaceOwned(owner)
	cause := errors.New("sysfs unavailable")
	deleteCalls := 0
	err := removeVethHostOwnedWith(name, owner, false, func() ([]net.Interface, error) {
		return []net.Interface{{Index: 91, Name: name}}, nil
	}, func(string) (string, error) {
		return "", cause
	}, func(string, int) error {
		deleteCalls++
		return nil
	})
	if !errors.Is(err, cause) {
		t.Fatalf("alias inspection cause not preserved: %v", err)
	}
	if deleteCalls != 0 {
		t.Fatalf("delete called %d times after alias inspection failure", deleteCalls)
	}
}

func TestRemoveVethHostOwnedTreatsDisappearanceAfterEnumerationAsClean(t *testing.T) {
	owner := "minicontainer:0123456789abcdef0123456789abcdef"
	name := VethHostIfaceOwned(owner)
	deleteCalls := 0
	err := removeVethHostOwnedWith(name, owner, false, func() ([]net.Interface, error) {
		return []net.Interface{{Index: 92, Name: name}}, nil
	}, func(string) (string, error) {
		return "", os.ErrNotExist
	}, func(string, int) error {
		deleteCalls++
		return nil
	})
	if err != nil {
		t.Fatalf("disappeared owned veth returned error: %v", err)
	}
	if deleteCalls != 0 {
		t.Fatalf("delete called %d times after disappearance", deleteCalls)
	}
}

func TestRemoveVethHostOwnedRejectsIndexReplacement(t *testing.T) {
	owner := "minicontainer:0123456789abcdef0123456789abcdef"
	name := VethHostIfaceOwned(owner)
	calls := 0
	deleteCalls := 0
	err := removeVethHostOwnedWith(name, owner, false, func() ([]net.Interface, error) {
		calls++
		index := 10
		if calls > 1 {
			index = 11
		}
		return []net.Interface{{Index: index, Name: name}}, nil
	}, func(string) (string, error) {
		return owner, nil
	}, func(string, int) error {
		deleteCalls++
		return nil
	})
	if err == nil {
		t.Fatal("replaced interface identity was accepted")
	}
	if deleteCalls != 0 {
		t.Fatalf("delete called %d times after identity replacement", deleteCalls)
	}
}

func TestRemoveVethHostOwnedRejectsOwnerChangeAtSameIdentity(t *testing.T) {
	owner := "minicontainer:0123456789abcdef0123456789abcdef"
	name := VethHostIfaceOwned(owner)
	aliasCalls := 0
	deleteCalls := 0
	err := removeVethHostOwnedWith(name, owner, false, func() ([]net.Interface, error) {
		return []net.Interface{{Index: 93, Name: name}}, nil
	}, func(string) (string, error) {
		aliasCalls++
		if aliasCalls == 1 {
			return owner, nil
		}
		return "minicontainer:foreign-owner", nil
	}, func(string, int) error {
		deleteCalls++
		return nil
	})
	if err == nil {
		t.Fatal("ownership change at same interface identity was accepted")
	}
	if aliasCalls != 2 {
		t.Fatalf("alias calls=%d, want 2", aliasCalls)
	}
	if deleteCalls != 0 {
		t.Fatalf("delete called %d times after ownership changed", deleteCalls)
	}
}
