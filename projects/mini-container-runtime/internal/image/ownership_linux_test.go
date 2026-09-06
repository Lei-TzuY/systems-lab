//go:build linux

package image

import (
	"errors"
	"testing"

	"golang.org/x/sys/unix"
)

func TestRestoreOwnershipWithRejectsNegativeIDs(t *testing.T) {
	calls := 0
	err := restoreOwnershipWith(3, -1, 0, 1000, func(int, int, int) error {
		calls++
		return nil
	})
	if err == nil {
		t.Fatal("negative tar ownership accepted")
	}
	if calls != 0 {
		t.Fatalf("fchown called %d times for invalid ownership", calls)
	}
}

func TestRestoreOwnershipWithIgnoresRootlessEPERM(t *testing.T) {
	err := restoreOwnershipWith(3, 0, 0, 1000, func(fd, uid, gid int) error {
		if fd != 3 || uid != 0 || gid != 0 {
			t.Fatalf("fchown args=%d %d:%d", fd, uid, gid)
		}
		return unix.EPERM
	})
	if err != nil {
		t.Fatalf("rootless EPERM should degrade to caller ownership: %v", err)
	}
}

func TestRestoreOwnershipWithKeepsPrivilegedEPERMFatal(t *testing.T) {
	err := restoreOwnershipWith(3, 123, 456, 0, func(int, int, int) error { return unix.EPERM })
	if !errors.Is(err, unix.EPERM) {
		t.Fatalf("privileged EPERM not preserved: %v", err)
	}
}

func TestRestoreOwnershipWithPropagatesUnexpectedFailure(t *testing.T) {
	cause := errors.New("fchown failed")
	err := restoreOwnershipWith(3, 123, 456, 1000, func(int, int, int) error { return cause })
	if !errors.Is(err, cause) {
		t.Fatalf("unexpected fchown failure not preserved: %v", err)
	}
}
