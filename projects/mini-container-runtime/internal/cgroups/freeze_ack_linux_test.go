//go:build linux

package cgroups

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestSetFreezeAtWaitsForKernelAcknowledgement(t *testing.T) {
	cgPath := filepath.Join(t.TempDir(), "cg")
	var wrotePath, wroteValue string
	reads := 0
	sleeps := 0

	err := setFreezeAt(
		cgPath,
		"1",
		true,
		4,
		time.Millisecond,
		func(path string, data []byte, mode os.FileMode) error {
			wrotePath = path
			wroteValue = string(data)
			if mode != 0o644 {
				t.Fatalf("write mode=%#o, want 0644", mode)
			}
			return nil
		},
		func(path string) ([]byte, error) {
			if path != filepath.Join(cgPath, "cgroup.events") {
				t.Fatalf("read path=%q", path)
			}
			reads++
			if reads < 3 {
				return []byte("populated 1\nfrozen 0\n"), nil
			}
			return []byte("populated 1\nfrozen 1\n"), nil
		},
		func(time.Duration) { sleeps++ },
	)
	if err != nil {
		t.Fatalf("setFreezeAt: %v", err)
	}
	if wrotePath != filepath.Join(cgPath, "cgroup.freeze") || wroteValue != "1" {
		t.Fatalf("write=(%q,%q), want cgroup.freeze=1", wrotePath, wroteValue)
	}
	if reads != 3 || sleeps != 2 {
		t.Fatalf("reads=%d sleeps=%d, want 3/2", reads, sleeps)
	}
}

func TestSetFreezeAtWaitsForThawAcknowledgement(t *testing.T) {
	reads := 0
	err := setFreezeAt(
		t.TempDir(),
		"0",
		false,
		3,
		0,
		func(string, []byte, os.FileMode) error { return nil },
		func(string) ([]byte, error) {
			reads++
			if reads == 1 {
				return []byte("frozen 1\n"), nil
			}
			return []byte("frozen 0\n"), nil
		},
		func(time.Duration) {},
	)
	if err != nil {
		t.Fatalf("setFreezeAt thaw: %v", err)
	}
	if reads != 2 {
		t.Fatalf("reads=%d, want 2", reads)
	}
}

func TestSetFreezeAtTimesOutWithoutKernelAcknowledgement(t *testing.T) {
	sleeps := 0
	err := setFreezeAt(
		t.TempDir(),
		"1",
		true,
		3,
		time.Millisecond,
		func(string, []byte, os.FileMode) error { return nil },
		func(string) ([]byte, error) { return []byte("frozen 0\n"), nil },
		func(time.Duration) { sleeps++ },
	)
	if err == nil || !strings.Contains(err.Error(), "timed out") {
		t.Fatalf("timeout error=%v", err)
	}
	if sleeps != 2 {
		t.Fatalf("sleeps=%d, want 2", sleeps)
	}
}

func TestSetFreezeAtSurfacesWriteAndAcknowledgementFailures(t *testing.T) {
	writeCause := errors.New("write denied")
	err := setFreezeAt(
		t.TempDir(), "1", true, 1, 0,
		func(string, []byte, os.FileMode) error { return writeCause },
		func(string) ([]byte, error) { t.Fatal("read after failed write"); return nil, nil },
		func(time.Duration) {},
	)
	if !errors.Is(err, writeCause) {
		t.Fatalf("write error=%v", err)
	}

	readCause := errors.New("cgroup disappeared")
	err = setFreezeAt(
		t.TempDir(), "1", true, 2, 0,
		func(string, []byte, os.FileMode) error { return nil },
		func(string) ([]byte, error) { return nil, readCause },
		func(time.Duration) {},
	)
	if !errors.Is(err, readCause) {
		t.Fatalf("ack read error=%v", err)
	}
}

func TestParseFrozenEventFailsClosedOnMalformedState(t *testing.T) {
	for _, tc := range []struct {
		name string
		data string
	}{
		{name: "missing", data: "populated 1\n"},
		{name: "malformed", data: "frozen\n"},
		{name: "invalid value", data: "frozen 2\n"},
		{name: "duplicate", data: "frozen 0\nfrozen 1\n"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			if _, err := parseFrozenEvent([]byte(tc.data)); err == nil {
				t.Fatalf("parseFrozenEvent accepted %q", tc.data)
			}
		})
	}
}

func TestParseFrozenEventIgnoresOtherCgroupEvents(t *testing.T) {
	frozen, err := parseFrozenEvent([]byte("populated 1\npressure 0\nfrozen 1\n"))
	if err != nil {
		t.Fatalf("parseFrozenEvent: %v", err)
	}
	if !frozen {
		t.Fatal("frozen 1 was not recognized")
	}
}
