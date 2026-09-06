package state

import (
	"encoding/json"
	"strings"
	"testing"
)

func TestStoppedGenerationLifecycleSnapshotIncludesRevisionAndStatus(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "lifecycle-snapshot"
	stopped := createModernStoppedForExitIdentityTest(t, st, id, 701, 7001)

	snapshot, err := st.readStoppedGenerationTeardownSnapshotUnlocked(id)
	if err != nil {
		t.Fatal(err)
	}
	if snapshot.status != StatusStopped || snapshot.revision != stopped.Revision {
		t.Fatalf("lifecycle coordinates status=%q revision=%d want stopped/%d", snapshot.status, snapshot.revision, stopped.Revision)
	}
	if !snapshot.identityEmbedded || snapshot.identity.PID != 701 || snapshot.identity.PIDStartTime != 7001 {
		t.Fatalf("unexpected teardown identity: %+v embedded=%v", snapshot.identity, snapshot.identityEmbedded)
	}
}

func TestStoppedGenerationStaleRevisionDoesNotInterpretNewerMalformedAuthority(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "stale-malformed-authority"
	stopped := createModernStoppedForExitIdentityTest(t, st, id, 702, 7002)
	staleRevision := stopped.Revision

	if err := st.MarkRunning(id, 703, 7003, stopped.CreatedAt.Add(2)); err != nil {
		t.Fatal(err)
	}
	rewriteStoppedGenerationRecordForTest(t, st, id, func(record map[string]json.RawMessage) {
		record["exit_identity_required"] = json.RawMessage(`"corrupt"`)
	})

	_, _, current, ok, required, err := st.GetStoppedExitIdentityPolicy(id, staleRevision)
	if err != nil {
		t.Fatalf("stale revision interpreted newer malformed teardown authority: %v", err)
	}
	if current || ok || required {
		t.Fatalf("stale result current=%v ok=%v required=%v", current, ok, required)
	}
}

func TestStoppedGenerationCurrentRevisionStillFailsClosedOnMalformedAuthority(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "current-malformed-authority"
	stopped := createModernStoppedForExitIdentityTest(t, st, id, 704, 7004)
	rewriteStoppedGenerationRecordForTest(t, st, id, func(record map[string]json.RawMessage) {
		record["exit_identity_required"] = json.RawMessage(`"corrupt"`)
	})

	_, _, current, ok, required, err := st.GetStoppedExitIdentityPolicy(id, stopped.Revision)
	if err == nil || !strings.Contains(err.Error(), "unmarshal persisted exit identity requirement") {
		t.Fatalf("expected current malformed authority error, got current=%v ok=%v required=%v err=%v", current, ok, required, err)
	}
	if !current || ok || required {
		t.Fatalf("malformed current generation did not fail closed: current=%v ok=%v required=%v", current, ok, required)
	}
}
