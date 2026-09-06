package state

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func rewriteAsPreSchemaStoppedFixture(t *testing.T, st *Store, id string) {
	t.Helper()
	path := filepath.Join(st.ctrDir, id+".json")
	data, err := readRegularStateFile(path, "container state")
	if err != nil {
		t.Fatal(err)
	}
	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		t.Fatal(err)
	}
	delete(raw, "state_schema_version")
	delete(raw, "legacy_dns_cleanup_authorized")
	data, err = json.MarshalIndent(raw, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	if err := atomicWriteFile(st.ctrDir, path, data); err != nil {
		t.Fatal(err)
	}
}

func TestContainerWritesStampCurrentStateSchemaVersion(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "writer-schema"
	if err := st.Save(&Container{ID: id, Status: StatusCreated, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	data, err := readRegularStateFile(filepath.Join(st.ctrDir, id+".json"), "container state")
	if err != nil {
		t.Fatal(err)
	}
	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		t.Fatal(err)
	}
	var version uint32
	if err := json.Unmarshal(raw["state_schema_version"], &version); err != nil {
		t.Fatalf("state writer schema was not persisted: %v", err)
	}
	if version != currentContainerStateSchemaVersion {
		t.Fatalf("state writer schema=%d want=%d", version, currentContainerStateSchemaVersion)
	}
}

func TestCurrentStoppedWritePublishesExplicitBroadCleanupAuthority(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "modern-broad-policy"
	if err := st.Save(&Container{ID: id, Status: StatusStopped, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	container, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}
	_, _, current, ok, required, err := st.GetStoppedExitIdentityPolicy(id, container.Revision)
	if err != nil {
		t.Fatal(err)
	}
	if !current || ok || required {
		t.Fatalf("unexpected explicit broad policy: current=%v ok=%v required=%v", current, ok, required)
	}

	data, err := readRegularStateFile(filepath.Join(st.ctrDir, id+".json"), "container state")
	if err != nil {
		t.Fatal(err)
	}
	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		t.Fatal(err)
	}
	var authorized bool
	if err := json.Unmarshal(raw["legacy_dns_cleanup_authorized"], &authorized); err != nil || !authorized {
		t.Fatalf("current stopped writer did not publish explicit broad policy: authorized=%v err=%v", authorized, err)
	}
}

func TestModernStoppedRecordWithoutAuthorityFailsClosed(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "modern-missing-authority"
	container := &Container{ID: id, Revision: 3, Status: StatusStopped, CreatedAt: time.Now()}
	record := struct {
		*Container
		StateSchemaVersion uint32 `json:"state_schema_version"`
	}{Container: container, StateSchemaVersion: currentContainerStateSchemaVersion}
	data, err := json.MarshalIndent(&record, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	if err := atomicWriteFile(st.ctrDir, filepath.Join(st.ctrDir, id+".json"), data); err != nil {
		t.Fatal(err)
	}

	_, _, current, ok, required, err := st.GetStoppedExitIdentityPolicy(id, container.Revision)
	if err == nil || !strings.Contains(err.Error(), "lacks explicit stopped-generation teardown authority") {
		t.Fatalf("expected modern missing authority to fail closed, got current=%v ok=%v required=%v err=%v", current, ok, required, err)
	}
	if !current || ok || required {
		t.Fatalf("modern missing authority acquired cleanup authority: current=%v ok=%v required=%v", current, ok, required)
	}
}

func TestFutureContainerStateSchemaFailsClosed(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "future-writer-schema"
	container := &Container{ID: id, Revision: 7, Status: StatusStopped, CreatedAt: time.Now()}
	record := struct {
		*Container
		StateSchemaVersion uint32 `json:"state_schema_version"`
	}{Container: container, StateSchemaVersion: currentContainerStateSchemaVersion + 1}
	data, err := json.MarshalIndent(&record, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	if err := atomicWriteFile(st.ctrDir, filepath.Join(st.ctrDir, id+".json"), data); err != nil {
		t.Fatal(err)
	}

	_, _, current, ok, required, err := st.GetStoppedExitIdentityPolicy(id, container.Revision)
	if err == nil || !strings.Contains(err.Error(), "unsupported container state schema version") {
		t.Fatalf("expected future writer schema to fail closed, got current=%v ok=%v required=%v err=%v", current, ok, required, err)
	}
	if !current || ok || required {
		t.Fatalf("future writer schema acquired teardown authority: current=%v ok=%v required=%v", current, ok, required)
	}
}
