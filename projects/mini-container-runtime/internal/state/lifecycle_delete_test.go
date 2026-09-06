package state

import (
	"testing"
	"time"
)

func TestLifecycleTransitionsDoNotRecreateDeletedState(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&Container{ID: "ctr-delete", Status: StatusCreated, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning("ctr-delete", 77, 88, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := st.Delete("ctr-delete"); err != nil {
		t.Fatal(err)
	}

	if _, err := st.MarkStoppedIfIdentity("ctr-delete", 77, 88, 0, time.Now()); err == nil {
		t.Fatal("expected deleted state to remain missing")
	}
	if _, err := st.Get("ctr-delete"); err == nil {
		t.Fatal("lifecycle update recreated deleted state")
	}
}
