package attach

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/logs"
	"minicontainer/internal/state"
)

func saveRunningAttachContainer(t *testing.T, st *state.Store, id string) *state.Container {
	t.Helper()
	c := &state.Container{
		ID:        id,
		Status:    state.StatusRunning,
		RootFS:    t.TempDir(),
		CreatedAt: time.Now(),
	}
	if err := st.Save(c); err != nil {
		t.Fatalf("Save container error: %v", err)
	}
	return c
}

func TestAttachContainer(t *testing.T) {
	t.Setenv("HOME", t.TempDir())
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}
	c := saveRunningAttachContainer(t, st, "ctr-attach-1")

	logFile, err := logs.CreateLogFile(c.ID)
	if err != nil {
		t.Fatalf("CreateLogFile: %v", err)
	}
	if _, err := logFile.WriteString("container log output line 1\n"); err != nil {
		t.Fatalf("write log: %v", err)
	}
	if err := logFile.Close(); err != nil {
		t.Fatalf("close log: %v", err)
	}

	var inBuf bytes.Buffer
	var outBuf bytes.Buffer
	if err := AttachContainer(st, c.ID, &inBuf, &outBuf); err != nil {
		t.Fatalf("AttachContainer error: %v", err)
	}
	if !strings.Contains(outBuf.String(), "container log output line 1") {
		t.Fatalf("Attached output missing expected log contents:\n%s", outBuf.String())
	}
}

func TestAttachContainerRejectsSymlinkedLog(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}
	c := saveRunningAttachContainer(t, st, "ctr-attach-link")

	outside := filepath.Join(t.TempDir(), "host-secret")
	const secret = "HOST-SECRET-MUST-NOT-LEAK"
	if err := os.WriteFile(outside, []byte(secret), 0o600); err != nil {
		t.Fatalf("write outside sentinel: %v", err)
	}
	logPath := logs.LogFilePath(c.ID)
	if err := os.MkdirAll(filepath.Dir(logPath), 0o700); err != nil {
		t.Fatalf("create log directory: %v", err)
	}
	if err := os.Symlink(outside, logPath); err != nil {
		t.Fatalf("create log symlink: %v", err)
	}

	var out bytes.Buffer
	err = AttachContainer(st, c.ID, bytes.NewReader(nil), &out)
	if err == nil {
		t.Fatal("AttachContainer followed symlinked log")
	}
	if strings.Contains(out.String(), secret) {
		t.Fatalf("AttachContainer leaked outside file contents: %q", out.String())
	}
	data, readErr := os.ReadFile(outside)
	if readErr != nil || string(data) != secret {
		t.Fatalf("outside sentinel changed: data=%q err=%v", data, readErr)
	}
}
