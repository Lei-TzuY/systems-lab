//go:build linux

package container

import (
	"errors"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"testing"
)

func TestCreateRuntimeHostsFileDisabledDoesNotAllocate(t *testing.T) {
	if file, err := createRuntimeHostsFile(false); err != nil || file != nil {
		t.Fatalf("disabled runtime hosts file=%v err=%v", file, err)
	}
}

func TestCreateRuntimeHostsFileIsUnlinkedRegularReadableFile(t *testing.T) {
	base := t.TempDir()
	content := "127.0.0.1\tlocalhost\n172.20.0.2\tapp\n"
	file, err := createRuntimeHostsFileWith(content, func(_ string, pattern string) (*os.File, error) {
		return os.CreateTemp(base, pattern)
	}, os.Remove)
	if err != nil {
		t.Fatalf("createRuntimeHostsFileWith: %v", err)
	}
	defer file.Close()

	if _, err := os.Stat(file.Name()); !os.IsNotExist(err) {
		t.Fatalf("runtime hosts pathname still exists: err=%v", err)
	}
	if err := validateRuntimeHostsFile(file); err != nil {
		t.Fatalf("validate anonymous file: %v", err)
	}
	data, err := os.ReadFile("/proc/self/fd/" + strconv.Itoa(int(file.Fd())))
	if err != nil {
		t.Fatalf("read anonymous hosts fd: %v", err)
	}
	if string(data) != content {
		t.Fatalf("content=%q want=%q", data, content)
	}
	info, err := file.Stat()
	if err != nil {
		t.Fatalf("stat: %v", err)
	}
	if info.Mode().Perm() != 0o644 {
		t.Fatalf("mode=%04o want 0644", info.Mode().Perm())
	}
}

func TestCreateRuntimeHostsFileFailureIsRuntimeControlAndCleansPath(t *testing.T) {
	base := t.TempDir()
	var path string
	cause := errors.New("unlink denied")
	file, err := createRuntimeHostsFileWith("hosts", func(_ string, pattern string) (*os.File, error) {
		f, err := os.CreateTemp(base, pattern)
		if err == nil {
			path = f.Name()
		}
		return f, err
	}, func(p string) error {
		if p == path {
			return cause
		}
		return os.Remove(p)
	})
	if file != nil || !errors.Is(err, cause) || !isRuntimeControlError(err) {
		t.Fatalf("unlink failure file=%v err=%v", file, err)
	}
	// The injected remover intentionally refuses cleanup; remove the fixture.
	_ = os.Remove(path)
}

func TestValidateRuntimeHostsFileRejectsLinkedAndNonRegularFiles(t *testing.T) {
	linked, err := os.CreateTemp(t.TempDir(), "hosts-*")
	if err != nil {
		t.Fatal(err)
	}
	defer linked.Close()
	if err := validateRuntimeHostsFile(linked); err == nil || !strings.Contains(err.Error(), "host link") {
		t.Fatalf("linked file error=%v", err)
	}

	dir, err := os.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer dir.Close()
	if err := validateRuntimeHostsFile(dir); err == nil || !strings.Contains(err.Error(), "not a regular file") {
		t.Fatalf("directory error=%v", err)
	}
}

func TestOpenRuntimeHostsTargetRequiresExistingRegularFileWithoutSymlinks(t *testing.T) {
	rootfs := t.TempDir()
	if err := os.MkdirAll(filepath.Join(rootfs, "etc"), 0o755); err != nil {
		t.Fatal(err)
	}
	hosts := filepath.Join(rootfs, "etc", "hosts")
	if err := os.WriteFile(hosts, []byte("original\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	fd, err := openRuntimeHostsTarget(rootfs)
	if err != nil {
		t.Fatalf("open regular hosts: %v", err)
	}
	_ = syscall.Close(fd)

	if err := os.Remove(hosts); err != nil {
		t.Fatal(err)
	}
	if _, err := openRuntimeHostsTarget(rootfs); err == nil {
		t.Fatal("missing /etc/hosts was created or accepted")
	}
	if _, err := os.Stat(hosts); !os.IsNotExist(err) {
		t.Fatalf("missing hosts target was mutated: %v", err)
	}

	outside := filepath.Join(t.TempDir(), "outside")
	if err := os.WriteFile(outside, []byte("outside"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, hosts); err != nil {
		t.Fatal(err)
	}
	if _, err := openRuntimeHostsTarget(rootfs); err == nil {
		t.Fatal("symlinked /etc/hosts accepted")
	}
}

func TestMountRuntimeHostsFileUsesFDPathsWithoutMutatingTarget(t *testing.T) {
	rootfs := t.TempDir()
	if err := os.MkdirAll(filepath.Join(rootfs, "etc"), 0o755); err != nil {
		t.Fatal(err)
	}
	hosts := filepath.Join(rootfs, "etc", "hosts")
	const original = "ORIGINAL-HOSTS\n"
	if err := os.WriteFile(hosts, []byte(original), 0o644); err != nil {
		t.Fatal(err)
	}

	source, err := os.CreateTemp(t.TempDir(), "source-*")
	if err != nil {
		t.Fatal(err)
	}
	defer source.Close()
	if err := os.Remove(source.Name()); err != nil {
		t.Fatal(err)
	}

	calls := 0
	err = mountRuntimeHostsFileWith(source, rootfs, false, func(src, dst, fs string, flags uintptr, data string) error {
		calls++
		if !strings.HasPrefix(src, "/proc/self/fd/") || !strings.HasPrefix(dst, "/proc/self/fd/") {
			t.Fatalf("mount paths src=%q dst=%q", src, dst)
		}
		if flags != syscall.MS_BIND || fs != "" || data != "" {
			t.Fatalf("mount args flags=%d fs=%q data=%q", flags, fs, data)
		}
		return nil
	})
	if err != nil {
		t.Fatalf("mountRuntimeHostsFileWith: %v", err)
	}
	if calls != 1 {
		t.Fatalf("mount calls=%d", calls)
	}
	data, err := os.ReadFile(hosts)
	if err != nil || string(data) != original {
		t.Fatalf("underlying hosts mutated: data=%q err=%v", data, err)
	}
}
