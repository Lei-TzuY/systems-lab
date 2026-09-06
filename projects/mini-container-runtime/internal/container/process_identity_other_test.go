//go:build !linux

package container

import "testing"

func TestProcessIdentityUnsupportedOffLinux(t *testing.T) {
	if _, err := ProcessStartTime(1); err == nil {
		t.Fatal("expected ProcessStartTime to be unsupported off Linux")
	}
	ok, err := ProcessIdentityMatches(1, 1)
	if err != nil {
		t.Fatalf("ProcessIdentityMatches returned unexpected error: %v", err)
	}
	if ok {
		t.Fatal("unsupported platform must not report identity match")
	}
}
