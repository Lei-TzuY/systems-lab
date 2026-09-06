package state

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func rewriteContainerStateSchemaForTest(t *testing.T, st *Store, id string, value json.RawMessage) {
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
	if value == nil {
		delete(raw, "state_schema_version")
	} else {
		raw["state_schema_version"] = value
	}
	data, err = json.MarshalIndent(raw, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	if err := atomicWriteFile(st.ctrDir, path, data); err != nil {
		t.Fatal(err)
	}
}

func newSchemaReadFixture(t *testing.T, st *Store, id string) *Container {
	t.Helper()
	c := &Container{ID: id, Status: StatusCreated, CreatedAt: time.Now()}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	return c
}

func TestContainerGetRejectsUnsupportedStateSchema(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "future-schema-read"
	newSchemaReadFixture(t, st, id)
	future, err := json.Marshal(currentContainerStateSchemaVersion + 1)
	if err != nil {
		t.Fatal(err)
	}
	rewriteContainerStateSchemaForTest(t, st, id, future)

	if _, err := st.Get(id); err == nil || !strings.Contains(err.Error(), "unsupported container state schema version") {
		t.Fatalf("Get accepted unsupported container state schema: %v", err)
	}
}

func TestContainerReadsRejectMalformedExplicitStateSchema(t *testing.T) {
	cases := []struct {
		name string
		raw  json.RawMessage
		want string
	}{
		{name: "zero", raw: json.RawMessage("0"), want: "invalid container state schema version 0"},
		{name: "null", raw: json.RawMessage("null"), want: "invalid container state schema version: null"},
		{name: "string", raw: json.RawMessage(`"1"`), want: "unmarshal container state schema version"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			st, err := Open(t.TempDir())
			if err != nil {
				t.Fatal(err)
			}
			defer st.Close()

			const id = "malformed-schema"
			newSchemaReadFixture(t, st, id)
			rewriteContainerStateSchemaForTest(t, st, id, tc.raw)

			if _, err := st.Get(id); err == nil || !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("Get accepted malformed explicit schema %s: %v", tc.name, err)
			}
		})
	}
}

func TestContainerPreSchemaReadCompatibility(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "legacy-schema-read"
	original := newSchemaReadFixture(t, st, id)
	rewriteContainerStateSchemaForTest(t, st, id, nil)

	got, err := st.Get(id)
	if err != nil {
		t.Fatalf("pre-schema container became unreadable: %v", err)
	}
	if got.ID != id || got.Revision != original.Revision || got.Status != StatusCreated {
		t.Fatalf("pre-schema container changed while decoding: %+v", got)
	}
}

func TestListAndResolveRejectUnsupportedStateSchema(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "future-list-resolve"
	newSchemaReadFixture(t, st, id)
	future, err := json.Marshal(currentContainerStateSchemaVersion + 1)
	if err != nil {
		t.Fatal(err)
	}
	rewriteContainerStateSchemaForTest(t, st, id, future)

	if _, err := st.List(); err == nil || !strings.Contains(err.Error(), "unsupported container state schema version") {
		t.Fatalf("List accepted unsupported container state schema: %v", err)
	}
	if _, err := st.Resolve("future-list"); err == nil || !strings.Contains(err.Error(), "unsupported container state schema version") {
		t.Fatalf("Resolve accepted unsupported container state schema: %v", err)
	}
}

func TestSaveCASRejectsUnsupportedCurrentStateSchema(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "future-schema-cas"
	c := newSchemaReadFixture(t, st, id)
	future, err := json.Marshal(currentContainerStateSchemaVersion + 1)
	if err != nil {
		t.Fatal(err)
	}
	rewriteContainerStateSchemaForTest(t, st, id, future)

	c.Health = "healthy"
	if err := st.Save(c); err == nil || !strings.Contains(err.Error(), "unsupported container state schema version") {
		t.Fatalf("Save CAS accepted unsupported current schema: %v", err)
	}

	data, err := readRegularStateFile(filepath.Join(st.ctrDir, id+".json"), "container state")
	if err != nil {
		t.Fatal(err)
	}
	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		t.Fatal(err)
	}
	if string(raw["state_schema_version"]) != string(future) {
		t.Fatalf("failed CAS rewrote unsupported schema: %s", raw["state_schema_version"])
	}
	if _, ok := raw["health"]; ok {
		t.Fatalf("failed CAS mutated unsupported state: health=%s", raw["health"])
	}
}
