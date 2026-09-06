package security

import (
	"os"
	"path/filepath"
	"testing"
)

func TestScanRootFS(t *testing.T) {
	tmpDir := t.TempDir()

	sshDir := filepath.Join(tmpDir, "root", ".ssh")
	_ = os.MkdirAll(sshDir, 0755)
	_ = os.WriteFile(filepath.Join(sshDir, "id_rsa"), []byte("-----BEGIN RSA PRIVATE KEY-----"), 0600)

	report, err := ScanRootFS(tmpDir)
	if err != nil {
		t.Fatalf("ScanRootFS error: %v", err)
	}

	if report.CriticalCount < 1 {
		t.Fatalf("ScanRootFS should detect leaked id_rsa key as CRITICAL issue")
	}

	if report.TotalIssues < 1 {
		t.Fatalf("TotalIssues = %d, want >= 1", report.TotalIssues)
	}
}
