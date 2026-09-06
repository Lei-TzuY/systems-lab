//go:build linux

package events

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

const testEventOne = `{"timestamp":"2026-09-02T00:00:00Z","type":"create","container_id":"one"}`
const testEventTwo = `{"timestamp":"2026-09-02T00:00:01Z","type":"start","container_id":"two"}`

func TestEventLogAppendTruncatesCrashTornTail(t *testing.T) {
	path := filepath.Join(t.TempDir(), "events.log")
	initial := testEventOne + "\n" + `{"timestamp":"2026-09-02T00:00:01Z","type":"start"`
	if err := os.WriteFile(path, []byte(initial), 0o600); err != nil {
		t.Fatal(err)
	}

	f, err := openEventLogForAppend(path)
	if err != nil {
		t.Fatalf("open append after torn tail: %v", err)
	}
	if _, err := fmt.Fprintln(f, testEventTwo); err != nil {
		_ = f.Close()
		t.Fatalf("append event: %v", err)
	}
	if err := f.Sync(); err != nil {
		_ = f.Close()
		t.Fatalf("sync event: %v", err)
	}
	if err := f.Close(); err != nil {
		t.Fatalf("close event log: %v", err)
	}

	got, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	want := testEventOne + "\n" + testEventTwo + "\n"
	if string(got) != want {
		t.Fatalf("event log after repair = %q, want %q", got, want)
	}
}

func TestEventLogAppendSalvagesCompleteUnterminatedRecord(t *testing.T) {
	path := filepath.Join(t.TempDir(), "events.log")
	if err := os.WriteFile(path, []byte(testEventOne), 0o600); err != nil {
		t.Fatal(err)
	}

	f, err := openEventLogForAppend(path)
	if err != nil {
		t.Fatalf("open append after missing newline: %v", err)
	}
	if _, err := fmt.Fprintln(f, testEventTwo); err != nil {
		_ = f.Close()
		t.Fatalf("append event: %v", err)
	}
	if err := f.Sync(); err != nil {
		_ = f.Close()
		t.Fatalf("sync event: %v", err)
	}
	if err := f.Close(); err != nil {
		t.Fatalf("close event log: %v", err)
	}

	got, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	want := testEventOne + "\n" + testEventTwo + "\n"
	if string(got) != want {
		t.Fatalf("event log after salvage = %q, want %q", got, want)
	}
}

func TestEventLogAppendRejectsCompleteInvalidUnterminatedRecord(t *testing.T) {
	path := filepath.Join(t.TempDir(), "events.log")
	initial := `{"timestamp":"2026-09-02T00:00:00Z","type":"create"}`
	if err := os.WriteFile(path, []byte(initial), 0o600); err != nil {
		t.Fatal(err)
	}

	f, err := openEventLogForAppend(path)
	if f != nil {
		_ = f.Close()
		t.Fatal("expected invalid unterminated event to be rejected")
	}
	if err == nil || !strings.Contains(err.Error(), "validate unterminated event record") {
		t.Fatalf("open error = %v, want validation failure", err)
	}
	got, readErr := os.ReadFile(path)
	if readErr != nil {
		t.Fatal(readErr)
	}
	if string(got) != initial {
		t.Fatalf("invalid complete record was mutated: got %q want %q", got, initial)
	}
}
