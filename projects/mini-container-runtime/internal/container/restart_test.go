package container

import (
	"testing"
)

func TestRestartPolicies(t *testing.T) {
	p1, err := ParseRestartPolicy("on-failure:3")
	if err != nil || p1.Type != RestartOnFailure || p1.MaxRetries != 3 {
		t.Fatalf("Parse on-failure:3 failed: %+v, err: %v", p1, err)
	}

	if !ShouldRestart(p1, 1, 0) {
		t.Fatalf("ShouldRestart on-failure retry 0 failed")
	}
	if !ShouldRestart(p1, 1, 2) {
		t.Fatalf("ShouldRestart on-failure retry 2 failed")
	}
	if ShouldRestart(p1, 1, 3) {
		t.Fatalf("ShouldRestart on-failure retry 3 should be false")
	}
	if ShouldRestart(p1, 0, 0) {
		t.Fatalf("ShouldRestart on exit code 0 should be false")
	}

	pAlways, _ := ParseRestartPolicy("always")
	if !ShouldRestart(pAlways, 0, 10) {
		t.Fatalf("ShouldRestart always should be true regardless of exit code")
	}
}
