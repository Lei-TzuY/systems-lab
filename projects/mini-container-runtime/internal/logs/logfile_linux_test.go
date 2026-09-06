//go:build linux

package logs

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"golang.org/x/sys/unix"
)

func useTemporaryLogHome(t *testing.T) string {
	t.Helper()
	home := t.TempDir()
	t.Setenv("HOME", home)
	return home
}

func logDirForHome(home string) string {
	return filepath.Join(home, ".minicontainer", "containers")
}

func TestContainerLogRejectsInvalidIDs(t *testing.T) {
	useTemporaryLogHome(t)
	invalid := []string{"", "   ", ".", "..", "../escape", "../../etc/passwd", "foo/bar", `foo\bar`, "colon:id"}
	for _, id := range invalid {
		if f, err := CreateLogFile(id); err == nil {
			f.Close()
			t.Fatalf("CreateLogFile(%q) succeeded", id)
		}
		var out bytes.Buffer
		if err := PrintLogs(id, 0, false, &out); err == nil {
			t.Fatalf("PrintLogs(%q) succeeded", id)
		}
	}
}

func TestContainerLogAppendReadAndPrivateModes(t *testing.T) {
	home := useTemporaryLogHome(t)
	dir := logDirForHome(home)
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(dir, 0o755); err != nil {
		t.Fatal(err)
	}

	const id = "abcdef1234567890"
	f, err := CreateLogFile(id)
	if err != nil {
		t.Fatalf("CreateLogFile: %v", err)
	}
	if _, err := f.WriteString("first\n"); err != nil {
		f.Close()
		t.Fatal(err)
	}
	if err := f.Close(); err != nil {
		t.Fatal(err)
	}

	path := filepath.Join(dir, id+".log")
	if err := os.Chmod(path, 0o777|os.ModeSetuid|os.ModeSetgid|os.ModeSticky); err != nil {
		t.Fatal(err)
	}

	f, err = CreateLogFile(id)
	if err != nil {
		t.Fatalf("reopen log: %v", err)
	}
	if _, err := f.WriteString("second\n"); err != nil {
		f.Close()
		t.Fatal(err)
	}
	if err := f.Close(); err != nil {
		t.Fatal(err)
	}

	var out bytes.Buffer
	if err := PrintLogs(id, 0, false, &out); err != nil {
		t.Fatalf("PrintLogs: %v", err)
	}
	if got := out.String(); got != "first\nsecond\n" {
		t.Fatalf("log contents=%q", got)
	}

	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if got := info.Mode().Perm(); got != 0o600 {
		t.Fatalf("log mode=%#o, want 0600", got)
	}
	if info.Mode()&(os.ModeSetuid|os.ModeSetgid|os.ModeSticky) != 0 {
		t.Fatalf("log retained special mode bits: %v", info.Mode())
	}
	dirInfo, err := os.Stat(dir)
	if err != nil {
		t.Fatal(err)
	}
	if got := dirInfo.Mode().Perm(); got != 0o700 {
		t.Fatalf("log dir mode=%#o, want 0700", got)
	}
}

func TestContainerLogRejectsSymlinkWithoutTouchingTarget(t *testing.T) {
	home := useTemporaryLogHome(t)
	dir := logDirForHome(home)
	if err := os.MkdirAll(dir, 0o700); err != nil {
		t.Fatal(err)
	}

	outside := filepath.Join(t.TempDir(), "outside.txt")
	const sentinel = "do-not-touch\n"
	if err := os.WriteFile(outside, []byte(sentinel), 0o600); err != nil {
		t.Fatal(err)
	}
	const id = "symlinked12345678"
	if err := os.Symlink(outside, filepath.Join(dir, id+".log")); err != nil {
		t.Fatal(err)
	}

	if f, err := CreateLogFile(id); err == nil {
		f.Close()
		t.Fatal("CreateLogFile followed symlink")
	}
	var out bytes.Buffer
	if err := PrintLogs(id, 0, false, &out); err == nil {
		t.Fatal("PrintLogs followed symlink")
	}
	if out.Len() != 0 {
		t.Fatalf("PrintLogs leaked symlink target contents: %q", out.String())
	}
	data, err := os.ReadFile(outside)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != sentinel {
		t.Fatalf("symlink target changed to %q", data)
	}
}

func TestContainerLogRejectsFIFOWithoutBlocking(t *testing.T) {
	home := useTemporaryLogHome(t)
	dir := logDirForHome(home)
	if err := os.MkdirAll(dir, 0o700); err != nil {
		t.Fatal(err)
	}
	const id = "fifo123456789012"
	path := filepath.Join(dir, id+".log")
	if err := unix.Mkfifo(path, 0o600); err != nil {
		t.Fatal(err)
	}
	if f, err := CreateLogFile(id); err == nil {
		f.Close()
		t.Fatal("CreateLogFile accepted FIFO")
	}
	var out bytes.Buffer
	if err := PrintLogs(id, 0, false, &out); err == nil || !strings.Contains(err.Error(), "regular file") {
		t.Fatalf("PrintLogs FIFO error=%v", err)
	}
}

func TestPrintLogsMissingShortIDDoesNotPanic(t *testing.T) {
	useTemporaryLogHome(t)
	var out bytes.Buffer
	err := PrintLogs("abc", 0, false, &out)
	if err == nil {
		t.Fatal("PrintLogs missing short ID succeeded")
	}
	if !strings.Contains(err.Error(), "abc") {
		t.Fatalf("missing-log error=%q, want short ID", err)
	}
}
