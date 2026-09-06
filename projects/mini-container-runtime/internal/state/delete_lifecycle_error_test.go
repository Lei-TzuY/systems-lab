package state

import (
	"errors"
	"testing"
	"time"
)

func TestDeleteIfNotRunningClassifiesRunningGeneration(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	c := &Container{
		ID:           "running-delete-classified",
		Status:       StatusRunning,
		PID:          7331,
		PIDStartTime: 144,
		CreatedAt:    time.Now(),
	}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}

	err = st.DeleteIfNotRunning(c.ID)
	if !errors.Is(err, ErrContainerRunning) {
		t.Fatalf("delete running error=%v, want ErrContainerRunning", err)
	}
	if _, getErr := st.Get(c.ID); getErr != nil {
		t.Fatalf("running container disappeared after classified refusal: %v", getErr)
	}
}
