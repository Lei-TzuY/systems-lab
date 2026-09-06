package security

import (
	"fmt"
	"io/fs"
	"path/filepath"
	"strings"
)

type Severity string

const (
	SeverityCritical Severity = "CRITICAL"
	SeverityWarning  Severity = "WARNING"
	SeverityInfo     Severity = "INFO"
)

type ScanIssue struct {
	Severity    Severity `json:"severity"`
	Category    string   `json:"category"`
	Path        string   `json:"path"`
	Description string   `json:"description"`
}

type ScanReport struct {
	Target        string      `json:"target"`
	TotalIssues   int         `json:"total_issues"`
	CriticalCount int         `json:"critical_count"`
	WarningCount  int         `json:"warning_count"`
	Issues        []ScanIssue `json:"issues"`
}

// ScanRootFS inspects container rootfs for security vulnerabilities (SUID binaries, plain-text SSH keys).
func ScanRootFS(rootfsPath string) (*ScanReport, error) {
	report := &ScanReport{
		Target: rootfsPath,
	}

	err := filepath.WalkDir(rootfsPath, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return nil
		}

		relPath, _ := filepath.Rel(rootfsPath, path)
		baseName := strings.ToLower(d.Name())

		// Check for private keys
		if !d.IsDir() && (strings.Contains(baseName, "id_rsa") || strings.HasSuffix(baseName, ".pem") || strings.Contains(baseName, "secret.key")) {
			report.Issues = append(report.Issues, ScanIssue{
				Severity:    SeverityCritical,
				Category:    "Private Key Leaked",
				Path:        relPath,
				Description: "Unmasked SSH or TLS private key found in container filesystem",
			})
			report.CriticalCount++
		}

		// Check for SUID / SGID binaries
		info, err := d.Info()
		if err == nil && !d.IsDir() {
			mode := info.Mode()
			if mode&fs.ModeSetuid != 0 || mode&fs.ModeSetgid != 0 {
				report.Issues = append(report.Issues, ScanIssue{
					Severity:    SeverityWarning,
					Category:    "SUID/SGID Binary",
					Path:        relPath,
					Description: fmt.Sprintf("File has SUID/SGID bit set: %s", mode),
				})
				report.WarningCount++
			}
		}

		return nil
	})

	report.TotalIssues = len(report.Issues)
	return report, err
}
