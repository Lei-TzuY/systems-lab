//go:build linux

package container

import (
	"fmt"
	"strings"
)

// validateSecurityProcessPolicy performs deterministic parent-side validation
// for security settings before the kernel creates the container generation.
// Actual capability and seccomp enforcement remains child-side immediately
// before payload exec.
func validateSecurityProcessPolicy(cfg Config) error {
	for _, raw := range cfg.CapDrop {
		name := strings.ToUpper(strings.TrimSpace(raw))
		if !strings.HasPrefix(name, "CAP_") {
			name = "CAP_" + name
		}
		if _, ok := capMap[name]; !ok {
			return fmt.Errorf("unknown capability %q", raw)
		}
	}
	if cfg.Seccomp && auditArch == 0 {
		return fmt.Errorf("seccomp is unsupported on this Linux architecture")
	}
	return nil
}
