package events

import (
	"errors"
	"os"
	"testing"
)

func TestOpenEventLogForStreamWithFollowRetriesInitialAbsence(t *testing.T) {
	f, err := os.CreateTemp(t.TempDir(), "events-")
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()

	calls := 0
	waits := 0
	got, err := openEventLogForStreamWith("ignored", true, func(string) (*os.File, error) {
		calls++
		if calls < 3 {
			return nil, os.ErrNotExist
		}
		return f, nil
	}, func() { waits++ })
	if err != nil {
		t.Fatalf("openEventLogForStreamWith: %v", err)
	}
	if got != f {
		t.Fatalf("opened file = %v, want %v", got, f)
	}
	if calls != 3 || waits != 2 {
		t.Fatalf("calls=%d waits=%d, want 3/2", calls, waits)
	}
}

func TestOpenEventLogForStreamWithNonFollowDoesNotRetryAbsence(t *testing.T) {
	calls := 0
	waits := 0
	got, err := openEventLogForStreamWith("ignored", false, func(string) (*os.File, error) {
		calls++
		return nil, os.ErrNotExist
	}, func() { waits++ })
	if got != nil {
		got.Close()
		t.Fatal("non-follow unexpectedly opened missing event log")
	}
	if !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("error=%v, want os.ErrNotExist", err)
	}
	if calls != 1 || waits != 0 {
		t.Fatalf("calls=%d waits=%d, want 1/0", calls, waits)
	}
}

func TestOpenEventLogForStreamWithFollowDoesNotRetrySafetyFailure(t *testing.T) {
	unsafeErr := errors.New("unsafe event log")
	calls := 0
	waits := 0
	got, err := openEventLogForStreamWith("ignored", true, func(string) (*os.File, error) {
		calls++
		return nil, unsafeErr
	}, func() { waits++ })
	if got != nil {
		got.Close()
		t.Fatal("follow unexpectedly opened unsafe event log")
	}
	if !errors.Is(err, unsafeErr) {
		t.Fatalf("error=%v, want safety failure", err)
	}
	if calls != 1 || waits != 0 {
		t.Fatalf("calls=%d waits=%d, want 1/0", calls, waits)
	}
}
