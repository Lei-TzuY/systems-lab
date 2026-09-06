//go:build linux

package container

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestStartContainerProcessRejectsInvalidCapabilityBeforeSpawn(t *testing.T) {
	marker := filepath.Join(t.TempDir(), "started")
	cmd := exec.Command("/bin/sh", "-c", "printf started > \"$1\"", "sh", marker)

	err := startContainerProcess(Config{CapDrop: []string{"CAP_NOT_REAL"}}, cmd)
	if err == nil {
		t.Fatal("startContainerProcess returned nil error for invalid capability")
	}
	if !strings.Contains(err.Error(), "unknown capability") {
		t.Fatalf("startContainerProcess error = %q, want unknown capability", err)
	}
	if cmd.Process != nil {
		t.Fatalf("process started despite invalid policy: PID=%d", cmd.Process.Pid)
	}
	if _, statErr := os.Stat(marker); !os.IsNotExist(statErr) {
		t.Fatalf("spawn marker exists after rejected admission: stat error = %v", statErr)
	}
}

func TestValidateSecurityProcessPolicyAcceptsKnownCapabilities(t *testing.T) {
	cfg := Config{CapDrop: []string{"net_raw", " CAP_SYS_ADMIN "}}
	if err := validateSecurityProcessPolicy(cfg); err != nil {
		t.Fatalf("validateSecurityProcessPolicy() error = %v", err)
	}
}

func TestValidateSecurityProcessPolicyAcceptsNativeSeccomp(t *testing.T) {
	if auditArch == 0 {
		t.Skip("seccomp audit architecture unsupported on this Linux architecture")
	}
	if err := validateSecurityProcessPolicy(Config{Seccomp: true}); err != nil {
		t.Fatalf("validateSecurityProcessPolicy(seccomp) error = %v", err)
	}
}
