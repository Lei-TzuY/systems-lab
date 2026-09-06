package events

import (
	"io"
	"os"
	"path/filepath"
	"testing"
)

func TestEventLogAppendCreatesPrivateRegularFile(t *testing.T) {
	path := filepath.Join(t.TempDir(), "state", "events.log")
	f, err := openEventLogForAppend(path)
	if err != nil {
		t.Fatalf("openEventLogForAppend: %v", err)
	}
	if _, err := io.WriteString(f, "event\n"); err != nil {
		f.Close()
		t.Fatalf("write: %v", err)
	}
	if err := f.Close(); err != nil {
		t.Fatalf("close: %v", err)
	}

	info, err := os.Stat(path)
	if err != nil {
		t.Fatalf("stat log: %v", err)
	}
	if !info.Mode().IsRegular() {
		t.Fatalf("mode = %v, want regular file", info.Mode())
	}
	if got := info.Mode().Perm(); got != 0o600 {
		t.Fatalf("permissions = %o, want 600", got)
	}

	dirInfo, err := os.Stat(filepath.Dir(path))
	if err != nil {
		t.Fatalf("stat directory: %v", err)
	}
	if got := dirInfo.Mode().Perm(); got != 0o700 {
		t.Fatalf("directory permissions = %o, want 700", got)
	}
}

func TestEventLogAppendRepairsLoosePermissions(t *testing.T) {
	path := filepath.Join(t.TempDir(), "events.log")
	if err := os.WriteFile(path, []byte("old\n"), 0o666); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(path, 0o666); err != nil {
		t.Fatal(err)
	}

	f, err := openEventLogForAppend(path)
	if err != nil {
		t.Fatalf("openEventLogForAppend: %v", err)
	}
	f.Close()

	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if got := info.Mode().Perm(); got != 0o600 {
		t.Fatalf("permissions = %o, want 600", got)
	}
}
