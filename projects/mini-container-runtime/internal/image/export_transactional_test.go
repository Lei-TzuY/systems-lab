package image

import (
	"archive/tar"
	"bytes"
	"compress/gzip"
	"io"
	"net"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestExportDirDestinationInsideSourceIsNotArchived(t *testing.T) {
	srcDir := t.TempDir()
	if err := os.WriteFile(filepath.Join(srcDir, "payload.txt"), []byte("payload\n"), 0644); err != nil {
		t.Fatalf("WriteFile payload: %v", err)
	}

	archivePath := filepath.Join(srcDir, "export.tar.gz")
	if err := os.WriteFile(archivePath, []byte("old archive sentinel"), 0600); err != nil {
		t.Fatalf("WriteFile old archive: %v", err)
	}

	if err := ExportDir(srcDir, archivePath); err != nil {
		t.Fatalf("ExportDir: %v", err)
	}

	names := readGzipTarNames(t, archivePath)
	if len(names) != 1 || names[0] != "payload.txt" {
		t.Fatalf("archive entries = %v, want only payload.txt", names)
	}
	for _, name := range names {
		if name == filepath.Base(archivePath) || strings.Contains(name, ".tmp-") {
			t.Fatalf("archive contains its own output artifact %q", name)
		}
	}
}

func TestExportDirFailurePreservesExistingDestinationAndCleansTemp(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("unix sockets are used to force a deterministic tar header failure")
	}

	srcDir := t.TempDir()
	if err := os.WriteFile(filepath.Join(srcDir, "payload.txt"), []byte("payload\n"), 0644); err != nil {
		t.Fatalf("WriteFile payload: %v", err)
	}
	listener, err := net.Listen("unix", filepath.Join(srcDir, "unsupported.sock"))
	if err != nil {
		t.Fatalf("Listen unix socket: %v", err)
	}
	defer listener.Close()

	destDir := t.TempDir()
	archivePath := filepath.Join(destDir, "export.tar.gz")
	original := []byte("previous-good-archive")
	if err := os.WriteFile(archivePath, original, 0600); err != nil {
		t.Fatalf("WriteFile existing destination: %v", err)
	}

	if err := ExportDir(srcDir, archivePath); err == nil {
		t.Fatal("ExportDir unexpectedly accepted an unsupported socket entry")
	}
	got, err := os.ReadFile(archivePath)
	if err != nil {
		t.Fatalf("ReadFile existing destination: %v", err)
	}
	if !bytes.Equal(got, original) {
		t.Fatalf("failed export changed destination: got %q want %q", got, original)
	}

	temps, err := filepath.Glob(filepath.Join(destDir, "."+filepath.Base(archivePath)+".tmp-*"))
	if err != nil {
		t.Fatalf("Glob temp archives: %v", err)
	}
	if len(temps) != 0 {
		t.Fatalf("failed export left temporary archives: %v", temps)
	}
}

func TestExportDirRejectsNonDirectorySourceWithoutTouchingDestination(t *testing.T) {
	base := t.TempDir()
	sourceFile := filepath.Join(base, "source.txt")
	if err := os.WriteFile(sourceFile, []byte("not a directory"), 0644); err != nil {
		t.Fatalf("WriteFile source: %v", err)
	}
	archivePath := filepath.Join(base, "export.tar")
	original := []byte("existing archive")
	if err := os.WriteFile(archivePath, original, 0600); err != nil {
		t.Fatalf("WriteFile destination: %v", err)
	}

	if err := ExportDir(sourceFile, archivePath); err == nil || !strings.Contains(err.Error(), "not a directory") {
		t.Fatalf("ExportDir non-directory error = %v", err)
	}
	got, err := os.ReadFile(archivePath)
	if err != nil {
		t.Fatalf("ReadFile destination: %v", err)
	}
	if !bytes.Equal(got, original) {
		t.Fatalf("invalid source changed destination: got %q want %q", got, original)
	}
}

func TestExportDirRejectsEmptyDestination(t *testing.T) {
	srcDir := t.TempDir()
	if err := os.WriteFile(filepath.Join(srcDir, "payload.txt"), []byte("payload\n"), 0644); err != nil {
		t.Fatalf("WriteFile payload: %v", err)
	}
	if err := ExportDir(srcDir, ""); err == nil || !strings.Contains(err.Error(), "archive path is empty") {
		t.Fatalf("ExportDir empty destination error = %v", err)
	}
}

func TestExportDirFollowsSourceDirectorySymlink(t *testing.T) {
	realSource := t.TempDir()
	if err := os.WriteFile(filepath.Join(realSource, "payload.txt"), []byte("payload\n"), 0644); err != nil {
		t.Fatalf("WriteFile payload: %v", err)
	}
	linkParent := t.TempDir()
	linkedSource := filepath.Join(linkParent, "source-link")
	if err := os.Symlink(realSource, linkedSource); err != nil {
		t.Skipf("Symlink unavailable: %v", err)
	}
	archivePath := filepath.Join(t.TempDir(), "export.tar.gz")

	if err := ExportDir(linkedSource, archivePath); err != nil {
		t.Fatalf("ExportDir symlinked source: %v", err)
	}
	if names := readGzipTarNames(t, archivePath); len(names) != 1 || names[0] != "payload.txt" {
		t.Fatalf("archive entries = %v, want payload.txt", names)
	}
}

func readGzipTarNames(t *testing.T, archivePath string) []string {
	t.Helper()
	f, err := os.Open(archivePath)
	if err != nil {
		t.Fatalf("Open archive: %v", err)
	}
	defer f.Close()
	gz, err := gzip.NewReader(f)
	if err != nil {
		t.Fatalf("NewReader gzip: %v", err)
	}
	defer gz.Close()

	tr := tar.NewReader(gz)
	var names []string
	for {
		hdr, err := tr.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			t.Fatalf("Next tar entry: %v", err)
		}
		names = append(names, hdr.Name)
	}
	return names
}
