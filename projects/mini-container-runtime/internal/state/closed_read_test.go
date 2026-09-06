package state

import (
	"errors"
	"testing"
	"time"
)

func TestClosedStoreReadsReturnErrStoreClosed(t *testing.T) {
	s, err := Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	if err := s.Close(); err != nil {
		t.Fatalf("Close: %v", err)
	}

	tests := []struct {
		name string
		run  func() error
	}{
		{
			name: "get",
			run: func() error {
				_, err := s.Get("closed-get")
				return err
			},
		},
		{
			name: "load",
			run: func() error {
				_, err := s.Load("closed-load")
				return err
			},
		},
		{
			name: "resolve",
			run: func() error {
				_, err := s.Resolve("closed-resolve")
				return err
			},
		},
		{
			name: "list",
			run: func() error {
				_, err := s.List()
				return err
			},
		},
		{
			name: "get-image",
			run: func() error {
				_, err := s.GetImage("closed:image")
				return err
			},
		},
		{
			name: "get-image-unlocked",
			run: func() error {
				_, err := s.GetImageUnlocked("closed:image")
				return err
			},
		},
		{
			name: "list-images",
			run: func() error {
				_, err := s.ListImages()
				return err
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if err := tt.run(); !errors.Is(err, ErrStoreClosed) {
				t.Fatalf("operation error = %v, want ErrStoreClosed", err)
			}
		})
	}
}

func TestGetImageUnlockedSerializesWithStoreLifecycle(t *testing.T) {
	s, err := Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer func() { _ = s.Close() }()

	// Hold the lifecycle mutex before starting the historically-unlocked read.
	// If the method bypasses the mutex, it can race Close and will return now.
	s.mu.Lock()
	done := make(chan error, 1)
	go func() {
		_, err := s.GetImageUnlocked("missing:image")
		done <- err
	}()

	select {
	case err := <-done:
		s.mu.Unlock()
		t.Fatalf("GetImageUnlocked bypassed Store lifecycle mutex: %v", err)
	case <-time.After(25 * time.Millisecond):
	}

	s.mu.Unlock()
	select {
	case err := <-done:
		if err == nil {
			t.Fatal("GetImageUnlocked unexpectedly found missing image")
		}
	case <-time.After(time.Second):
		t.Fatal("GetImageUnlocked did not resume after lifecycle mutex release")
	}
}
