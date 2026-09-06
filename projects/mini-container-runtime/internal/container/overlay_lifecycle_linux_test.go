//go:build linux

package container

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestCreateParentOverlayWorkDirDisabledDoesNotAllocate(t *testing.T) {
	calls := 0
	dir, err := createParentOverlayWorkDir(false, func(string, string) (string, error) {
		calls++
		return "unexpected", nil
	})
	if err != nil {
		t.Fatalf("disabled allocation returned error: %v", err)
	}
	if dir != "" || calls != 0 {
		t.Fatalf("disabled allocation dir=%q calls=%d", dir, calls)
	}
}

func TestCreateParentOverlayWorkDirFailureIsRuntimeControl(t *testing.T) {
	cause := errors.New("tmp storage unavailable")
	_, err := createParentOverlayWorkDir(true, func(dir, pattern string) (string, error) {
		if dir != "" || pattern != overlayWorkDirPrefix+"*" {
			t.Fatalf("mkdir args dir=%q pattern=%q", dir, pattern)
		}
		return "", cause
	})
	if !errors.Is(err, cause) {
		t.Fatalf("allocation cause not preserved: %v", err)
	}
	if !isRuntimeControlError(err) {
		t.Fatalf("allocation failure not classified as runtime control: %v", err)
	}
}

func TestCreateParentOverlayWorkDirRejectsNilAndEmptyAllocator(t *testing.T) {
	if _, err := createParentOverlayWorkDir(true, nil); err == nil || !isRuntimeControlError(err) {
		t.Fatalf("nil allocator error=%v", err)
	}
	if _, err := createParentOverlayWorkDir(true, func(string, string) (string, error) { return "", nil }); err == nil || !isRuntimeControlError(err) {
		t.Fatalf("empty allocator result error=%v", err)
	}
}

func TestClearRuntimeControlEnvironmentRemovesAmbientNamespaceOnly(t *testing.T) {
	for key, value := range map[string]string{
		sentinelEnvKey:                  "1",
		execSentinelKey:                 "1",
		execStartTimeKey:                "123",
		overlayWorkDirEnv:               "/tmp/owned",
		"MINICONTAINER_DEBUG":          "1",
		"MINICONTAINER_FUTURE_CONTROL": "secret",
	} {
		t.Setenv(key, value)
	}
	t.Setenv("MINI_CONTAINER_USER_VALUE", "keep")
	t.Setenv("PATH", "/bin:/usr/bin")

	if err := clearRuntimeControlEnvironment(); err != nil {
		t.Fatalf("clearRuntimeControlEnvironment: %v", err)
	}

	for _, key := range []string{
		sentinelEnvKey,
		execSentinelKey,
		execStartTimeKey,
		overlayWorkDirEnv,
		"MINICONTAINER_DEBUG",
		"MINICONTAINER_FUTURE_CONTROL",
	} {
		if _, ok := os.LookupEnv(key); ok {
			t.Fatalf("runtime control environment %q survived isolation", key)
		}
	}
	if got := os.Getenv("MINI_CONTAINER_USER_VALUE"); got != "keep" {
		t.Fatalf("non-runtime environment changed: %q", got)
	}
	if got := os.Getenv("PATH"); got != "/bin:/usr/bin" {
		t.Fatalf("ordinary PATH changed: %q", got)
	}
}

func TestConsumeOverlayWorkDirAcceptsPrivateParentDirectoryAndClearsEnv(t *testing.T) {
	base := t.TempDir()
	dir, err := os.MkdirTemp(base, overlayWorkDirPrefix+"*")
	if err != nil {
		t.Fatalf("MkdirTemp: %v", err)
	}
	if err := os.Chmod(dir, 0o700); err != nil {
		t.Fatalf("chmod: %v", err)
	}
	t.Setenv(overlayWorkDirEnv, dir)
	t.Setenv(sentinelEnvKey, "1")
	t.Setenv("MINICONTAINER_FUTURE_CONTROL", "ambient")

	got, err := consumeOverlayWorkDir(true)
	if err != nil {
		t.Fatalf("consumeOverlayWorkDir: %v", err)
	}
	if got != dir {
		t.Fatalf("got workdir %q, want %q", got, dir)
	}
	for _, key := range []string{overlayWorkDirEnv, sentinelEnvKey, "MINICONTAINER_FUTURE_CONTROL"} {
		if _, ok := os.LookupEnv(key); ok {
			t.Fatalf("runtime environment %q leaked after consumption", key)
		}
	}
}

func TestConsumeOverlayWorkDirDisabledStillClearsReservedEnv(t *testing.T) {
	t.Setenv(overlayWorkDirEnv, "/forged/value")
	t.Setenv(sentinelEnvKey, "1")
	got, err := consumeOverlayWorkDir(false)
	if err != nil {
		t.Fatalf("disabled consume: %v", err)
	}
	if got != "" {
		t.Fatalf("disabled consume returned %q", got)
	}
	for _, key := range []string{overlayWorkDirEnv, sentinelEnvKey} {
		if _, ok := os.LookupEnv(key); ok {
			t.Fatalf("reserved runtime environment %q leaked for non-overlay payload", key)
		}
	}
}

func TestConsumeOverlayWorkDirRejectsMissingRelativeAndUnexpectedNames(t *testing.T) {
	t.Setenv(overlayWorkDirEnv, "")
	if _, err := consumeOverlayWorkDir(true); err == nil || !strings.Contains(err.Error(), "did not provide") {
		t.Fatalf("missing workdir error=%v", err)
	}

	t.Setenv(overlayWorkDirEnv, "relative/path")
	if _, err := consumeOverlayWorkDir(true); err == nil || !strings.Contains(err.Error(), "not absolute") {
		t.Fatalf("relative workdir error=%v", err)
	}

	dir := t.TempDir()
	t.Setenv(overlayWorkDirEnv, dir)
	if _, err := consumeOverlayWorkDir(true); err == nil || !strings.Contains(err.Error(), "unexpected name") {
		t.Fatalf("unexpected-name workdir error=%v", err)
	}
}

func TestConsumeOverlayWorkDirRejectsSymlinkAndNonPrivateDirectory(t *testing.T) {
	base := t.TempDir()
	realDir, err := os.MkdirTemp(base, overlayWorkDirPrefix+"real-*")
	if err != nil {
		t.Fatalf("MkdirTemp real: %v", err)
	}
	link := filepath.Join(base, overlayWorkDirPrefix+"link")
	if err := os.Symlink(realDir, link); err != nil {
		t.Fatalf("Symlink: %v", err)
	}
	t.Setenv(overlayWorkDirEnv, link)
	if _, err := consumeOverlayWorkDir(true); err == nil || !strings.Contains(err.Error(), "not a real directory") {
		t.Fatalf("symlink workdir error=%v", err)
	}

	publicDir, err := os.MkdirTemp(base, overlayWorkDirPrefix+"public-*")
	if err != nil {
		t.Fatalf("MkdirTemp public: %v", err)
	}
	if err := os.Chmod(publicDir, 0o755); err != nil {
		t.Fatalf("chmod public: %v", err)
	}
	t.Setenv(overlayWorkDirEnv, publicDir)
	if _, err := consumeOverlayWorkDir(true); err == nil || !strings.Contains(err.Error(), "not private") {
		t.Fatalf("public workdir error=%v", err)
	}
}

func TestFinishOverlayWorkDirSuccessPreservesExistingResult(t *testing.T) {
	payloadErr := errors.New("payload exit")
	calls := 0
	got := finishOverlayWorkDir(payloadErr, "/tmp/owned", func(path string) error {
		calls++
		if path != "/tmp/owned" {
			t.Fatalf("remove path=%q", path)
		}
		return nil
	})
	if calls != 1 {
		t.Fatalf("remove calls=%d", calls)
	}
	if got != payloadErr {
		t.Fatalf("successful cleanup changed result: %v", got)
	}
}

func TestFinishOverlayWorkDirCleanupFailureJoinsPayloadAndBlocksRestart(t *testing.T) {
	payloadErr := errors.New("payload exit 17")
	cleanupCause := errors.New("remove denied")
	got := finishOverlayWorkDir(payloadErr, "/tmp/owned", func(string) error { return cleanupCause })
	if !errors.Is(got, payloadErr) || !errors.Is(got, cleanupCause) {
		t.Fatalf("joined result=%v", got)
	}
	if !isRuntimeControlError(got) {
		t.Fatalf("cleanup failure not classified as runtime control: %v", got)
	}
}

func TestFinishOverlayWorkDirNilRemoverFailsClosed(t *testing.T) {
	got := finishOverlayWorkDir(nil, "/tmp/owned", nil)
	if got == nil || !isRuntimeControlError(got) {
		t.Fatalf("nil remover result=%v", got)
	}
}

func TestFinishOverlayWorkDirEmptyPathDoesNotRemove(t *testing.T) {
	calls := 0
	payloadErr := errors.New("payload")
	got := finishOverlayWorkDir(payloadErr, "", func(string) error {
		calls++
		return nil
	})
	if got != payloadErr || calls != 0 {
		t.Fatalf("empty path result=%v calls=%d", got, calls)
	}
}
