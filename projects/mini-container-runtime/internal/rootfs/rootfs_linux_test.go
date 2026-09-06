//go:build linux

package rootfs

import (
	"errors"
	"os"
	"strings"
	"syscall"
	"testing"
)

func TestIsolateWithPivotFailsClosed(t *testing.T) {
	cause := errors.New("pivot unavailable")
	calls := 0

	err := isolateWithPivot("/fake/root", false, func(newRoot string, debug bool) error {
		calls++
		if newRoot != "/fake/root" {
			t.Fatalf("newRoot=%q", newRoot)
		}
		if debug {
			t.Fatal("unexpected debug=true")
		}
		return cause
	})

	if calls != 1 {
		t.Fatalf("pivot calls=%d, want exactly 1", calls)
	}
	if !errors.Is(err, cause) {
		t.Fatalf("pivot failure not preserved: %v", err)
	}
	if !strings.Contains(err.Error(), "pivot_root isolation required") {
		t.Fatalf("failure does not explain fail-closed requirement: %v", err)
	}
}

func TestIsolateWithPivotSucceedsOnlyAfterPivotSuccess(t *testing.T) {
	calls := 0
	err := isolateWithPivot("/fake/root", true, func(newRoot string, debug bool) error {
		calls++
		if newRoot != "/fake/root" || !debug {
			t.Fatalf("unexpected arguments: root=%q debug=%v", newRoot, debug)
		}
		return nil
	})
	if err != nil {
		t.Fatalf("isolateWithPivot: %v", err)
	}
	if calls != 1 {
		t.Fatalf("pivot calls=%d, want 1", calls)
	}
}

func TestIsolateWithPivotRejectsNilImplementation(t *testing.T) {
	err := isolateWithPivot("/fake/root", false, nil)
	if err == nil || !strings.Contains(err.Error(), "pivot_root isolation function is nil") {
		t.Fatalf("nil pivot error=%v", err)
	}
}

func successfulPivotRootOps() pivotRootOps {
	return pivotRootOps{
		mount: func(source, target, fstype string, flags uintptr, data string) error { return nil },
		mkdir: func(path string, mode os.FileMode) error { return nil },
		pivot: func(newRoot, putOld string) error { return nil },
		chdir: func(path string) error { return nil },
		unmount: func(target string, flags int) error { return nil },
		remove: func(path string) error { return nil },
	}
}

func TestPivotRootRejectsForeignPivotDirectoryAndRollsBackBind(t *testing.T) {
	ops := successfulPivotRootOps()
	pivotCalls := 0
	removeCalls := 0
	var unmountTargets []string
	ops.mkdir = func(path string, mode os.FileMode) error {
		return syscall.EEXIST
	}
	ops.pivot = func(newRoot, putOld string) error {
		pivotCalls++
		return nil
	}
	ops.remove = func(path string) error {
		removeCalls++
		return nil
	}
	ops.unmount = func(target string, flags int) error {
		unmountTargets = append(unmountTargets, target)
		return nil
	}

	err := pivotRootWithOps("/fake/root", false, ops)
	if !errors.Is(err, syscall.EEXIST) {
		t.Fatalf("error=%v, want EEXIST", err)
	}
	if pivotCalls != 0 || removeCalls != 0 {
		t.Fatalf("foreign pivot directory touched: pivot=%d remove=%d", pivotCalls, removeCalls)
	}
	if len(unmountTargets) != 1 || unmountTargets[0] != "/fake/root" {
		t.Fatalf("bind rollback targets=%v, want [/fake/root]", unmountTargets)
	}
}

func TestPivotRootFailureJoinsOwnedDirectoryAndBindRollbackFailures(t *testing.T) {
	pivotErr := errors.New("pivot failed")
	removeErr := errors.New("remove failed")
	unmountErr := errors.New("unmount failed")
	ops := successfulPivotRootOps()
	var removed []string
	var unmounted []string
	ops.pivot = func(newRoot, putOld string) error { return pivotErr }
	ops.remove = func(path string) error {
		removed = append(removed, path)
		return removeErr
	}
	ops.unmount = func(target string, flags int) error {
		unmounted = append(unmounted, target)
		return unmountErr
	}

	err := pivotRootWithOps("/fake/root", false, ops)
	if !errors.Is(err, pivotErr) || !errors.Is(err, removeErr) || !errors.Is(err, unmountErr) {
		t.Fatalf("error=%v, want setup and both rollback causes", err)
	}
	if len(removed) != 1 || removed[0] != "/fake/root/.pivot_old" {
		t.Fatalf("removed=%v", removed)
	}
	if len(unmounted) != 1 || unmounted[0] != "/fake/root" {
		t.Fatalf("unmounted=%v", unmounted)
	}
}

func TestPivotRootChdirFailureDetachesOldRootAndRemovesOwnedDirectory(t *testing.T) {
	cause := errors.New("chdir failed")
	ops := successfulPivotRootOps()
	var unmounted []string
	var removed []string
	ops.chdir = func(path string) error { return cause }
	ops.unmount = func(target string, flags int) error {
		unmounted = append(unmounted, target)
		return nil
	}
	ops.remove = func(path string) error {
		removed = append(removed, path)
		return nil
	}

	err := pivotRootWithOps("/fake/root", false, ops)
	if !errors.Is(err, cause) {
		t.Fatalf("error=%v, want chdir cause", err)
	}
	if len(unmounted) != 1 || unmounted[0] != "/.pivot_old" {
		t.Fatalf("unmounted=%v", unmounted)
	}
	if len(removed) != 1 || removed[0] != "/.pivot_old" {
		t.Fatalf("removed=%v", removed)
	}
}

func TestPivotRootRetriesOldRootDetachDuringFailureRollback(t *testing.T) {
	cause := errors.New("detach failed")
	ops := successfulPivotRootOps()
	unmountCalls := 0
	removeCalls := 0
	ops.unmount = func(target string, flags int) error {
		if target != "/.pivot_old" {
			t.Fatalf("unexpected unmount target %q", target)
		}
		unmountCalls++
		if unmountCalls == 1 {
			return cause
		}
		return nil
	}
	ops.remove = func(path string) error {
		removeCalls++
		return nil
	}

	err := pivotRootWithOps("/fake/root", false, ops)
	if !errors.Is(err, cause) {
		t.Fatalf("error=%v, want detach cause", err)
	}
	if unmountCalls != 2 {
		t.Fatalf("unmount calls=%d, want initial attempt plus rollback retry", unmountCalls)
	}
	if removeCalls != 1 {
		t.Fatalf("remove calls=%d, want cleanup after successful retry", removeCalls)
	}
}

func TestPivotRootDoesNotIgnoreOwnedDirectoryRemovalFailure(t *testing.T) {
	cause := errors.New("rmdir failed")
	ops := successfulPivotRootOps()
	removeCalls := 0
	ops.remove = func(path string) error {
		if path != "/.pivot_old" {
			t.Fatalf("unexpected remove path %q", path)
		}
		removeCalls++
		if removeCalls == 1 {
			return cause
		}
		return nil
	}

	err := pivotRootWithOps("/fake/root", false, ops)
	if !errors.Is(err, cause) {
		t.Fatalf("error=%v, want rmdir cause", err)
	}
	if removeCalls != 2 {
		t.Fatalf("remove calls=%d, want checked removal plus rollback retry", removeCalls)
	}
}
