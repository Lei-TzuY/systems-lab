//go:build linux

package container

import (
	"errors"
	"strings"
	"syscall"
	"testing"
)

func TestReadOnlyRootDisabledDoesNotMount(t *testing.T) {
	calls := 0
	err := enforceReadOnlyRootWithMount(false, false, func(source, target, fstype string, flags uintptr, data string) error {
		calls++
		return errors.New("must not be called")
	})
	if err != nil {
		t.Fatalf("disabled read-only root returned error: %v", err)
	}
	if calls != 0 {
		t.Fatalf("mount calls=%d, want 0", calls)
	}
}

func TestReadOnlyRootFailureIsFatal(t *testing.T) {
	cause := errors.New("remount rejected")
	calls := 0
	err := enforceReadOnlyRootWithMount(true, false, func(source, target, fstype string, flags uintptr, data string) error {
		calls++
		if source != "" || target != "/" || fstype != "" || data != "" {
			t.Fatalf("unexpected mount args: source=%q target=%q fstype=%q data=%q", source, target, fstype, data)
		}
		wantFlags := uintptr(syscall.MS_BIND | syscall.MS_REMOUNT | syscall.MS_RDONLY)
		if flags != wantFlags {
			t.Fatalf("flags=%#x, want %#x", flags, wantFlags)
		}
		return cause
	})
	if calls != 1 {
		t.Fatalf("mount calls=%d, want 1", calls)
	}
	if !errors.Is(err, cause) {
		t.Fatalf("remount failure not preserved: %v", err)
	}
	if !strings.Contains(err.Error(), "remount root read-only") {
		t.Fatalf("error lacks read-only context: %v", err)
	}
}

func TestReadOnlyRootSuccessUsesRequiredFlags(t *testing.T) {
	calls := 0
	err := enforceReadOnlyRootWithMount(true, true, func(source, target, fstype string, flags uintptr, data string) error {
		calls++
		wantFlags := uintptr(syscall.MS_BIND | syscall.MS_REMOUNT | syscall.MS_RDONLY)
		if source != "" || target != "/" || fstype != "" || data != "" || flags != wantFlags {
			t.Fatalf("unexpected mount call: %q %q %q %#x %q", source, target, fstype, flags, data)
		}
		return nil
	})
	if err != nil {
		t.Fatalf("successful remount returned error: %v", err)
	}
	if calls != 1 {
		t.Fatalf("mount calls=%d, want 1", calls)
	}
}

func TestReadOnlyRootRejectsNilMountImplementation(t *testing.T) {
	err := enforceReadOnlyRootWithMount(true, false, nil)
	if err == nil || !strings.Contains(err.Error(), "read-only root remount function is nil") {
		t.Fatalf("nil mount error=%v", err)
	}
}
