package state

import (
	"bytes"
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func rewriteContainerIDForTest(t *testing.T, st *Store, storageID, payloadID string) {
	t.Helper()
	path := filepath.Join(st.ctrDir, storageID+".json")
	data, err := readRegularStateFile(path, "container state")
	if err != nil {
		t.Fatal(err)
	}
	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		t.Fatal(err)
	}
	encodedID, err := json.Marshal(payloadID)
	if err != nil {
		t.Fatal(err)
	}
	raw["id"] = encodedID
	data, err = json.MarshalIndent(raw, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	if err := atomicWriteFile(st.ctrDir, path, data); err != nil {
		t.Fatal(err)
	}
}

func TestContainerReadsRejectStoragePayloadIdentityMismatch(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const storageID = "record-alpha"
	c := &Container{ID: storageID, Status: StatusCreated, CreatedAt: time.Now()}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	rewriteContainerIDForTest(t, st, storageID, "record-beta")

	assertMismatch := func(name string, err error) {
		t.Helper()
		if err == nil || !strings.Contains(err.Error(), "container state identity mismatch") {
			t.Fatalf("%s accepted cross-record container identity: %v", name, err)
		}
	}

	_, err = st.Get(storageID)
	assertMismatch("Get", err)

	_, err = st.List()
	assertMismatch("List", err)

	_, err = st.Resolve("record-al")
	assertMismatch("Resolve", err)
}

func TestSaveCASRejectsStoragePayloadIdentityMismatchWithoutMutation(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const storageID = "cas-alpha"
	c := &Container{ID: storageID, Status: StatusCreated, CreatedAt: time.Now()}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	rewriteContainerIDForTest(t, st, storageID, "cas-beta")

	path := filepath.Join(st.ctrDir, storageID+".json")
	before, err := readRegularStateFile(path, "container state")
	if err != nil {
		t.Fatal(err)
	}

	c.Health = "healthy"
	err = st.Save(c)
	if err == nil || !strings.Contains(err.Error(), "container state identity mismatch") {
		t.Fatalf("Save CAS accepted cross-record container identity: %v", err)
	}

	after, err := readRegularStateFile(path, "container state")
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(before, after) {
		t.Fatalf("failed CAS mutated mismatched record\nbefore=%s\nafter=%s", before, after)
	}
	if _, err := readRegularStateFile(filepath.Join(st.ctrDir, "cas-beta.json"), "container state"); err == nil {
		t.Fatal("failed CAS created a record named by the corrupted payload id")
	}
}
