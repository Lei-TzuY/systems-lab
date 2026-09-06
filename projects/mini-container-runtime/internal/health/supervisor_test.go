package health

import (
	"context"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestHealthSupervisor(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	ctr := &state.Container{
		ID:        "ctr-health-test",
		Status:    state.StatusRunning,
		Health:    StatusStarting,
		RootFS:    tmpDir,
		CreatedAt: time.Now(),
	}
	if err := st.Save(ctr); err != nil {
		t.Fatalf("Save container error: %v", err)
	}

	failCount := 0
	checkFn := func(ctx context.Context) (int, error) {
		if failCount < 2 {
			failCount++
			return 1, nil
		}
		return 0, nil
	}

	sup := NewSupervisor("ctr-health-test", Config{
		Interval: 10 * time.Millisecond,
		Timeout:  50 * time.Millisecond,
		Retries:  3,
	}, checkFn, st)

	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()

	done := make(chan struct{})
	go func() {
		defer close(done)
		sup.Start(ctx)
	}()

	var finalErr string
	for {
		updated, err := st.Get("ctr-health-test")
		if err == nil && updated.Health == StatusHealthy {
			break // Success, cancel and wait for exit
		}
		select {
		case <-ctx.Done():
			if err != nil {
				finalErr = "Get container error: " + err.Error()
			} else {
				finalErr = "Expected container status healthy within deadline, got " + updated.Health
			}
			break
		case <-time.After(10 * time.Millisecond):
		}
		if finalErr != "" {
			break
		}
	}

	cancel() // Trigger supervisor exit
	<-done   // Ensure supervisor has completely returned before test ends

	if finalErr != "" {
		t.Fatal(finalErr)
	}
}
