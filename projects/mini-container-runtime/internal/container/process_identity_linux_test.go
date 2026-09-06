//go:build linux

package container

import (
	"os"
	"strconv"
	"strings"
	"testing"
)

func TestParseProcessStartTimeHandlesTrickyCommandNames(t *testing.T) {
	// After the parenthesized comm, tokens begin at proc stat field 3. Place the
	// desired starttime at field 22 => token index 19 in the suffix.
	fields := []string{"S"}
	for i := 0; i < 18; i++ {
		fields = append(fields, strconv.Itoa(i+1))
	}
	fields = append(fields, "987654321")
	stat := "123 (worker ) with spaces) " + strings.Join(fields, " ") + " 0 0\n"

	got, err := parseProcessStartTime(stat)
	if err != nil {
		t.Fatalf("parseProcessStartTime: %v", err)
	}
	if got != 987654321 {
		t.Fatalf("starttime = %d, want 987654321", got)
	}
}

func TestParseProcessStartTimeRejectsMalformedInput(t *testing.T) {
	for _, stat := range []string{
		"123 no-parens",
		"123 (cmd) S 1 2 3",
		"123 (cmd) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 nope",
	} {
		if _, err := parseProcessStartTime(stat); err == nil {
			t.Fatalf("expected malformed stat %q to fail", stat)
		}
	}
}

func TestProcessIdentityMatchesCurrentProcess(t *testing.T) {
	pid := os.Getpid()
	start, err := ProcessStartTime(pid)
	if err != nil {
		t.Fatalf("ProcessStartTime: %v", err)
	}
	if start == 0 {
		t.Fatal("current process starttime must be non-zero")
	}
	ok, err := ProcessIdentityMatches(pid, start)
	if err != nil {
		t.Fatalf("ProcessIdentityMatches: %v", err)
	}
	if !ok {
		t.Fatal("current process identity should match")
	}
	ok, err = ProcessIdentityMatches(pid, start+1)
	if err != nil {
		t.Fatalf("ProcessIdentityMatches stale identity: %v", err)
	}
	if ok {
		t.Fatal("wrong starttime unexpectedly matched")
	}
}
