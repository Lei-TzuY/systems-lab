//go:build linux

package network

import (
	"errors"
	"strings"
	"testing"
)

func TestRemovePortForwardingAttemptsBothRulesAndJoinsFailures(t *testing.T) {
	preCause := errors.New("prerouting delete failed")
	outCause := errors.New("output delete failed")
	var calls []string
	err := removePortForwardingWith(8080, 80, "172.20.0.2", "", false, func(args ...string) ([]byte, error) {
		calls = append(calls, strings.Join(args, " "))
		if len(calls) == 1 {
			return []byte("pre"), preCause
		}
		return []byte("out"), outCause
	})
	if len(calls) != 2 {
		t.Fatalf("calls=%v, want both delete rules attempted", calls)
	}
	if !strings.Contains(calls[0], "-D PREROUTING -p tcp --dport 8080") {
		t.Fatalf("first cleanup rule=%s", calls[0])
	}
	if !strings.Contains(calls[1], "-D OUTPUT -p tcp") {
		t.Fatalf("second cleanup rule=%s", calls[1])
	}
	if !errors.Is(err, preCause) || !errors.Is(err, outCause) {
		t.Fatalf("cleanup errors not joined: %v", err)
	}
}

func TestRemovePortForwardingRejectsNilRunner(t *testing.T) {
	if err := removePortForwardingWith(1, 1, "127.0.0.1", "tcp", false, nil); err == nil {
		t.Fatal("nil runner unexpectedly accepted")
	}
}
