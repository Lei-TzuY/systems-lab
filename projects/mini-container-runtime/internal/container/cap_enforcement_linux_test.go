//go:build linux

package container

import (
	"fmt"
	"os"
	"os/exec"
	"syscall"
	"testing"
)

const prCapBsetRead = 23

func capabilityInBoundingSet(cap uintptr) (bool, error) {
	value, _, errno := syscall.RawSyscall(syscall.SYS_PRCTL, prCapBsetRead, cap, 0)
	if errno != 0 {
		return false, errno
	}
	return value == 1, nil
}

func TestDropCapabilitiesEitherEnforcesPolicyOrFailsClosed(t *testing.T) {
	if os.Getenv("MINICONTAINER_CAP_DROP_HELPER") == "1" {
		const cap = uintptr(0) // CAP_CHOWN: supported by every capabilities-aware Linux kernel.

		before, err := capabilityInBoundingSet(cap)
		if err != nil {
			fmt.Fprintf(os.Stderr, "read bounding set before drop: %v\n", err)
			os.Exit(90)
		}
		if !before {
			// The requested policy is already satisfied. There is no enforcement
			// failure to distinguish in this process environment.
			os.Exit(0)
		}

		dropErr := DropCapabilities([]string{"CAP_CHOWN"}, false)
		after, err := capabilityInBoundingSet(cap)
		if err != nil {
			fmt.Fprintf(os.Stderr, "read bounding set after drop: %v\n", err)
			os.Exit(91)
		}
		if dropErr == nil && after {
			fmt.Fprintln(os.Stderr, "capability drop reported success but CAP_CHOWN remained in bounding set")
			os.Exit(92)
		}
		if dropErr != nil && !after {
			fmt.Fprintf(os.Stderr, "capability drop changed kernel state but returned error: %v\n", dropErr)
			os.Exit(93)
		}
		os.Exit(0)
	}

	cmd := exec.Command(os.Args[0], "-test.run=^TestDropCapabilitiesEitherEnforcesPolicyOrFailsClosed$")
	cmd.Env = append(os.Environ(), "MINICONTAINER_CAP_DROP_HELPER=1")
	if output, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("capability enforcement helper failed: %v\n%s", err, output)
	}
}
