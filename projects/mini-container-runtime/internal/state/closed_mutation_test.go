package state

import (
	"errors"
	"os"
	"testing"
)

func TestClosedStoreMutationsReturnErrStoreClosed(t *testing.T) {
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
			name: "save",
			run: func() error {
				return s.Save(&Container{ID: "closed-save"})
			},
		},
		{
			name: "delete",
			run: func() error {
				return s.Delete("closed-delete")
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

func TestClosedStoreMutationDoesNotUseRecycledDescriptor(t *testing.T) {
	s, err := Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	if err := s.Close(); err != nil {
		t.Fatalf("Close: %v", err)
	}
	if s.lockFile != nil {
		t.Fatal("Close retained state lock handle")
	}

	// Encourage the OS to recycle recently closed descriptor numbers. A
	// post-Close mutation must consult the nil Store handle and fail before it
	// can flock any unrelated descriptor that happens to reuse the old number.
	var files []*os.File
	for i := 0; i < 32; i++ {
		f, err := os.OpenFile(os.DevNull, os.O_RDWR, 0)
		if err != nil {
			t.Fatalf("open decoy fd %d: %v", i, err)
		}
		files = append(files, f)
	}
	defer func() {
		for _, f := range files {
			_ = f.Close()
		}
	}()

	if err := s.Save(&Container{ID: "fd-reuse"}); !errors.Is(err, ErrStoreClosed) {
		t.Fatalf("Save after descriptor recycling = %v, want ErrStoreClosed", err)
	}
}

func TestNilStateLockIsClosedStoreSentinel(t *testing.T) {
	if err := lockStateFile(nil); !errors.Is(err, ErrStoreClosed) {
		t.Fatalf("lockStateFile(nil) = %v, want ErrStoreClosed", err)
	}
}
