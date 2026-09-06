package state

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func writeRawCurrentImageMetadata(t *testing.T, store *Store, key, data string) string {
	t.Helper()
	path := filepath.Join(store.imgDir, imageMetadataFilename(key))
	if err := os.WriteFile(path, []byte(data), 0o600); err != nil {
		t.Fatal(err)
	}
	return path
}

func TestReadImageMetadataRejectsUnknownFields(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	path := writeRawCurrentImageMetadata(t, store, "strict:test", `{"name":"strict:test","rootfs":"/root","loaded_at":"0001-01-01T00:00:00Z","unexpected":true}`)
	if _, err := readImageMetadata(path); err == nil || !strings.Contains(err.Error(), "unknown field") {
		t.Fatalf("unknown-field error=%v", err)
	}
}

func TestReadImageMetadataRejectsDuplicateIdentityKey(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	path := writeRawCurrentImageMetadata(t, store, "victim:test", `{"name":"attacker:test","name":"victim:test","rootfs":"/root","loaded_at":"0001-01-01T00:00:00Z"}`)
	if _, err := readImageMetadata(path); err == nil || !strings.Contains(err.Error(), `duplicate JSON object key "name"`) {
		t.Fatalf("duplicate-key error=%v", err)
	}
}

func TestValidateUniqueJSONKeysRejectsNestedDuplicates(t *testing.T) {
	err := validateUniqueJSONKeys([]byte(`{"outer":{"value":1,"value":2}}`))
	if err == nil || !strings.Contains(err.Error(), `duplicate JSON object key "value"`) {
		t.Fatalf("nested duplicate-key error=%v", err)
	}
}

func TestReadImageMetadataRejectsTrailingJSONValue(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	path := writeRawCurrentImageMetadata(t, store, "strict:test", `{"name":"strict:test","rootfs":"/root","loaded_at":"0001-01-01T00:00:00Z"} {}`)
	if _, err := readImageMetadata(path); err == nil || !strings.Contains(err.Error(), "trailing JSON") {
		t.Fatalf("trailing-value error=%v", err)
	}
}

func TestDecodeImageMetadataStrictAcceptsCurrentSchema(t *testing.T) {
	var img Image
	data := []byte(`{"id":"sha256:abc","repository":"repo/app","tag":"latest","name":"repo/app:latest","rootfs":"/root","size":42,"loaded_at":"2026-08-31T00:00:00Z","work_dir":"/work","env":["A=B"],"cmd":["/bin/sh"],"exposed_ports":["8080/tcp"]}`)
	if err := decodeImageMetadataStrict(data, &img); err != nil {
		t.Fatalf("decode current schema: %v", err)
	}
	if img.Name != "repo/app:latest" || img.RootFS != "/root" || img.Size != 42 {
		t.Fatalf("decoded image=%+v", img)
	}
}
