package state

import (
	"errors"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestOwnershipReadsRejectClosedStoreAfterDescriptorReuse(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-sidecar-closed"
	saveCreatedContainer(t, st, id)
	if err := st.MarkRunning(id, 101, 202, time.Now()); err != nil {
		t.Fatal(err)
	}
	cgroup := CgroupOwnership{
		Name:         "minicontainer-ctr-sidecar-closed-101-202",
		PID:          101,
		PIDStartTime: 202,
	}
	if err := st.MarkCgroupOwnedIfIdentity(id, cgroup.PID, cgroup.PIDStartTime, cgroup.Name); err != nil {
		t.Fatal(err)
	}
	network := testNetworkOwnership(101, 202)
	if err := st.MarkNetworkOwnedIfIdentity(id, network); err != nil {
		t.Fatal(err)
	}
	if err := st.Close(); err != nil {
		t.Fatal(err)
	}

	// Encourage the OS to recycle descriptors formerly owned by Store. Closed
	// Store reads must be rejected before any stale path can reach an unrelated
	// descriptor generation.
	decoyPath := filepath.Join(t.TempDir(), "decoy")
	if err := os.WriteFile(decoyPath, []byte("decoy"), 0o600); err != nil {
		t.Fatal(err)
	}
	var decoys []*os.File
	for i := 0; i < 64; i++ {
		f, err := os.Open(decoyPath)
		if err != nil {
			t.Fatal(err)
		}
		decoys = append(decoys, f)
	}
	defer func() {
		for _, f := range decoys {
			_ = f.Close()
		}
	}()

	if got, ok, err := st.GetCgroupOwnership(id); !errors.Is(err, ErrStoreClosed) || ok || got != (CgroupOwnership{}) {
		t.Fatalf("GetCgroupOwnership after Close: got=%+v ok=%v err=%v", got, ok, err)
	}
	if got, ok, err := st.GetNetworkOwnership(id); !errors.Is(err, ErrStoreClosed) || ok || !networkOwnershipEqual(got, NetworkOwnership{}) {
		t.Fatalf("GetNetworkOwnership after Close: got=%+v ok=%v err=%v", got, ok, err)
	}
}

func TestOwnershipReadsPreferClosedSentinelOverMissingSidecar(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Close(); err != nil {
		t.Fatal(err)
	}

	if _, ok, err := st.GetCgroupOwnership("missing"); !errors.Is(err, ErrStoreClosed) || ok {
		t.Fatalf("GetCgroupOwnership missing after Close: ok=%v err=%v", ok, err)
	}
	if _, ok, err := st.GetNetworkOwnership("missing"); !errors.Is(err, ErrStoreClosed) || ok {
		t.Fatalf("GetNetworkOwnership missing after Close: ok=%v err=%v", ok, err)
	}
}
