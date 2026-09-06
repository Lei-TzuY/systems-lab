//go:build linux

package container

import (
	"os"
	"testing"

	"golang.org/x/sys/unix"
)

func TestPrepareExecThreadClearsEntireRuntimeControlNamespace(t *testing.T) {
	for key, value := range map[string]string{
		execSentinelKey:                  "1",
		execStartTimeKey:                 "123",
		execStartedFDKey:                 "9",
		"MINICONTAINER_INIT":            "1",
		"MINICONTAINER_DEBUG":           "1",
		"MINICONTAINER_FUTURE_CONTROL":  "secret",
	} {
		t.Setenv(key, value)
	}
	t.Setenv("MINI_CONTAINER_USER_VALUE", "keep")
	t.Setenv("PATH", "/bin:/usr/bin")

	calls := 0
	if err := prepareExecThreadWith(func(flags int) error {
		calls++
		if flags != unix.CLONE_FS {
			t.Fatalf("unshare flags=%#x, want %#x", flags, unix.CLONE_FS)
		}
		return nil
	}); err != nil {
		t.Fatalf("prepareExecThreadWith: %v", err)
	}
	if calls != 1 {
		t.Fatalf("unshare calls=%d, want 1", calls)
	}

	for _, key := range []string{
		execSentinelKey,
		execStartTimeKey,
		execStartedFDKey,
		"MINICONTAINER_INIT",
		"MINICONTAINER_DEBUG",
		"MINICONTAINER_FUTURE_CONTROL",
	} {
		if _, ok := os.LookupEnv(key); ok {
			t.Fatalf("runtime control environment %q survived exec isolation", key)
		}
	}
	if got := os.Getenv("MINI_CONTAINER_USER_VALUE"); got != "keep" {
		t.Fatalf("non-runtime environment changed: %q", got)
	}
	if got := os.Getenv("PATH"); got != "/bin:/usr/bin" {
		t.Fatalf("ordinary PATH changed: %q", got)
	}
}

func TestPrepareExecThreadDoesNotClearRuntimeEnvironmentWhenUnshareFails(t *testing.T) {
	t.Setenv("MINICONTAINER_FUTURE_CONTROL", "secret")
	cause := unix.EPERM

	err := prepareExecThreadWith(func(int) error { return cause })
	if err == nil {
		t.Fatal("expected unshare failure")
	}
	if got := os.Getenv("MINICONTAINER_FUTURE_CONTROL"); got != "secret" {
		t.Fatalf("runtime environment changed before successful thread isolation: %q", got)
	}
}
