//go:build linux

package logs

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

func TestContainerLogRejectsSymlinkedLogDirectory(t *testing.T) {
	home := useTemporaryLogHome(t)
	stateDir := filepath.Join(home, ".minicontainer")
	if err := os.MkdirAll(stateDir, 0o700); err != nil {
		t.Fatal(err)
	}
	outsideDir := t.TempDir()
	logDir := filepath.Join(stateDir, "containers")
	if err := os.Symlink(outsideDir, logDir); err != nil {
		t.Fatal(err)
	}

	const id = "dirlink123456789"
	if f, err := CreateLogFile(id); err == nil {
		f.Close()
		t.Fatal("CreateLogFile accepted symlinked log directory")
	}
	if _, err := os.Lstat(filepath.Join(outsideDir, id+".log")); !os.IsNotExist(err) {
		t.Fatalf("log was created through symlinked directory: err=%v", err)
	}

	var out bytes.Buffer
	if err := PrintLogs(id, 0, false, &out); err == nil {
		t.Fatal("PrintLogs accepted symlinked log directory")
	}
}
