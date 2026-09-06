//go:build linux

package dns

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"golang.org/x/sys/unix"
)

func TestReadDNSRegistryFileAtRejectsOversizedSparseRegistry(t *testing.T) {
	dir := t.TempDir()
	const networkName = "default"
	name := networkName + ".json"
	path := filepath.Join(dir, name)
	file, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := file.Truncate(maxDNSRegistryBytes + 1); err != nil {
		file.Close()
		t.Fatal(err)
	}
	if err := file.Close(); err != nil {
		t.Fatal(err)
	}

	dirFD, err := unix.Open(dir, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		t.Fatal(err)
	}
	defer unix.Close(dirFD)

	if _, exists, err := readDNSRegistryFileAt(dirFD, name, networkName); err == nil || !strings.Contains(err.Error(), "exceeds") {
		t.Fatalf("oversized registry=(exists=%v err=%v), want fail-closed size rejection", exists, err)
	}
}
