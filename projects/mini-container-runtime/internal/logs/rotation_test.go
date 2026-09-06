package logs

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestRotateLogFileRetainsExactTailAndSecuresMode(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log")
	const content = "12345678901234567890"
	if err := os.WriteFile(logPath, []byte(content), 0o666); err != nil {
		t.Fatal(err)
	}

	if err := RotateLogFile(logPath, 10); err != nil {
		t.Fatalf("RotateLogFile error: %v", err)
	}

	data, err := os.ReadFile(logPath)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != content[len(content)-10:] {
		t.Fatalf("rotated data = %q, want %q", data, content[len(content)-10:])
	}
	info, err := os.Stat(logPath)
	if err != nil {
		t.Fatal(err)
	}
	if got := info.Mode().Perm(); got != 0o600 {
		t.Fatalf("rotated log mode = %o, want 600", got)
	}
}

func TestRotateLogFileLeavesSmallLogUnchanged(t *testing.T) {
	logPath := filepath.Join(t.TempDir(), "small.log")
	const content = "small"
	if err := os.WriteFile(logPath, []byte(content), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := RotateLogFile(logPath, 64); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(logPath)
	if err != nil || string(data) != content {
		t.Fatalf("small log changed: data=%q err=%v", data, err)
	}
}

func TestRotateLogFileMissingIsNoOp(t *testing.T) {
	if err := RotateLogFile(filepath.Join(t.TempDir(), "missing.log"), 10); err != nil {
		t.Fatalf("missing log rotation = %v, want nil", err)
	}
}

func TestRotateLogFileRejectsSymlinkTarget(t *testing.T) {
	dir := t.TempDir()
	outside := filepath.Join(t.TempDir(), "host-secret")
	const secret = "HOST-SECRET-MUST-STAY-UNCHANGED"
	if err := os.WriteFile(outside, []byte(secret), 0o600); err != nil {
		t.Fatal(err)
	}
	logPath := filepath.Join(dir, "container.log")
	if err := os.Symlink(outside, logPath); err != nil {
		t.Fatal(err)
	}

	err := RotateLogFile(logPath, 4)
	if err == nil {
		t.Fatal("RotateLogFile followed symlinked log")
	}
	data, readErr := os.ReadFile(outside)
	if readErr != nil || string(data) != secret {
		t.Fatalf("outside sentinel changed: data=%q err=%v", data, readErr)
	}
	info, lstatErr := os.Lstat(logPath)
	if lstatErr != nil || info.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("log symlink was replaced: info=%v err=%v", info, lstatErr)
	}
}

func TestRotateLogFileRejectsSymlinkedDirectory(t *testing.T) {
	outsideDir := t.TempDir()
	outsideLog := filepath.Join(outsideDir, "container.log")
	const secret = "outside-log-content"
	if err := os.WriteFile(outsideLog, []byte(secret), 0o600); err != nil {
		t.Fatal(err)
	}
	root := t.TempDir()
	linkedDir := filepath.Join(root, "containers")
	if err := os.Symlink(outsideDir, linkedDir); err != nil {
		t.Fatal(err)
	}

	err := RotateLogFile(filepath.Join(linkedDir, "container.log"), 3)
	if err == nil {
		t.Fatal("RotateLogFile accepted symlinked log directory")
	}
	data, readErr := os.ReadFile(outsideLog)
	if readErr != nil || string(data) != secret {
		t.Fatalf("outside log changed: data=%q err=%v", data, readErr)
	}
}

func TestRotateLogFileLargeTail(t *testing.T) {
	logPath := filepath.Join(t.TempDir(), "large.log")
	content := strings.Repeat("0123456789abcdef", 32*1024)
	const keep = int64(100000)
	if err := os.WriteFile(logPath, []byte(content), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := RotateLogFile(logPath, keep); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(logPath)
	if err != nil {
		t.Fatal(err)
	}
	want := content[len(content)-int(keep):]
	if string(data) != want {
		t.Fatalf("large rotated tail mismatch: got %d bytes, want %d", len(data), len(want))
	}
}
