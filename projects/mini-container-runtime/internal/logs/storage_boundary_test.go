package logs

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

func TestManagedLogStorageRejectsSymlinkedStateRoot(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)

	outside := t.TempDir()
	outsideContainers := filepath.Join(outside, "containers")
	if err := os.MkdirAll(outsideContainers, 0o700); err != nil {
		t.Fatal(err)
	}
	const secret = "HOST-LOG-SECRET-MUST-NOT-BE-TOUCHED"
	victim := filepath.Join(outsideContainers, "victim.log")
	if err := os.WriteFile(victim, []byte(secret), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, filepath.Join(home, ".minicontainer")); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}

	if f, err := CreateLogFile("new"); err == nil {
		_ = f.Close()
		t.Fatal("CreateLogFile accepted symlinked state root")
	}
	if _, err := os.Lstat(filepath.Join(outsideContainers, "new.log")); !os.IsNotExist(err) {
		t.Fatalf("CreateLogFile wrote through symlinked state root: %v", err)
	}

	var out bytes.Buffer
	if err := PrintLogs("victim", 0, false, &out); err == nil {
		t.Fatal("PrintLogs accepted symlinked state root")
	}
	if bytes.Contains(out.Bytes(), []byte(secret)) {
		t.Fatalf("PrintLogs leaked outside log contents: %q", out.String())
	}

	if err := RotateLogFile(LogFilePath("victim"), 4); err == nil {
		t.Fatal("RotateLogFile accepted symlinked state root")
	}
	data, err := os.ReadFile(victim)
	if err != nil || string(data) != secret {
		t.Fatalf("outside sentinel changed: data=%q err=%v", data, err)
	}
}

func TestManagedLogStorageCreatesPrivateRealDirectories(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)

	f, err := CreateLogFile("private-log")
	if err != nil {
		t.Fatalf("CreateLogFile: %v", err)
	}
	if _, err := f.WriteString("hello\n"); err != nil {
		t.Fatal(err)
	}
	if err := f.Close(); err != nil {
		t.Fatal(err)
	}

	for _, path := range []string{managedLogStateDir(), managedLogDir()} {
		info, err := os.Lstat(path)
		if err != nil {
			t.Fatalf("Lstat(%s): %v", path, err)
		}
		if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
			t.Fatalf("managed directory %s is not a real directory: %v", path, info.Mode())
		}
		if got := info.Mode().Perm(); got != 0o700 {
			t.Fatalf("managed directory %s mode=%o, want 700", path, got)
		}
	}
	info, err := os.Lstat(LogFilePath("private-log"))
	if err != nil {
		t.Fatal(err)
	}
	if !info.Mode().IsRegular() || info.Mode().Perm() != 0o600 {
		t.Fatalf("managed log mode=%v, want regular 0600", info.Mode())
	}
}
