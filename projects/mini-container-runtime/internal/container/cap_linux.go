//go:build linux

// internal/container/cap_linux.go
//
// Linux Capabilities Management (`--cap-drop` / `--cap-add`)
// ─────────────────────────────────────────────────────────────
// Linux capabilities break down the all-powerful `root` privilege into ~41
// fine-grained privileges (e.g. CAP_NET_ADMIN, CAP_SYS_PTRACE, CAP_SYS_ADMIN).
//
// Docker drops ~21 dangerous capabilities by default.
//
// Capability Bounding Set
// ───────────────────────
// Dropping a capability from the Capability Bounding Set (PR_CAPBSET_DROP)
// prevents the process and all its children from ever gaining that capability,
// even if they run set-uid binaries.

package container

import (
	"fmt"
	"strings"
	"syscall"
)

const (
	prCapBsetDrop = 24 // PR_CAPBSET_DROP
)

// Known Linux capability constants mapped by uppercase string name.
var capMap = map[string]uintptr{
	"CAP_CHOWN":            0,
	"CAP_DAC_OVERRIDE":     1,
	"CAP_DAC_READ_SEARCH":  2,
	"CAP_FOWNER":           3,
	"CAP_FSETID":           4,
	"CAP_KILL":             5,
	"CAP_SETGID":           6,
	"CAP_SETUID":           7,
	"CAP_SETPCAP":          8,
	"CAP_LINUX_IMMUTABLE":  9,
	"CAP_NET_BIND_SERVICE": 10,
	"CAP_NET_BROADCAST":    11,
	"CAP_NET_ADMIN":        12,
	"CAP_NET_RAW":          13,
	"CAP_IPC_LOCK":         14,
	"CAP_IPC_OWNER":        15,
	"CAP_SYS_MODULE":       16,
	"CAP_SYS_RAWIO":        17,
	"CAP_SYS_CHROOT":       18,
	"CAP_SYS_PTRACE":       19,
	"CAP_SYS_PACCT":        20,
	"CAP_SYS_ADMIN":        21,
	"CAP_SYS_BOOT":         22,
	"CAP_SYS_NICE":         23,
	"CAP_SYS_RESOURCE":     24,
	"CAP_SYS_TIME":         25,
	"CAP_SYS_TTY_CONFIG":   26,
	"CAP_MKNOD":            27,
	"CAP_LEASE":            28,
	"CAP_AUDIT_WRITE":      29,
	"CAP_AUDIT_CONTROL":    30,
	"CAP_SETFCAP":          31,
	"CAP_MAC_OVERRIDE":     32,
	"CAP_MAC_ADMIN":        33,
	"CAP_SYSLOG":           34,
	"CAP_WAKE_ALARM":       35,
	"CAP_BLOCK_SUSPEND":    36,
	"CAP_AUDIT_READ":       37,
}

// DropCapabilities drops specified capabilities from the bounding set.
func DropCapabilities(capNames []string, debug bool) error {
	for _, raw := range capNames {
		name := strings.ToUpper(strings.TrimSpace(raw))
		if !strings.HasPrefix(name, "CAP_") {
			name = "CAP_" + name
		}

		capVal, ok := capMap[name]
		if !ok {
			return fmt.Errorf("unknown capability %q", raw)
		}

		// A requested capability policy is a security boundary. Never treat a
		// kernel refusal as success: EPERM means the runtime lacked the authority
		// to enforce the drop, while EINVAL means the running kernel does not
		// understand that capability. In both cases continuing would silently
		// weaken the caller's requested process policy.
		if _, _, errno := syscall.RawSyscall(syscall.SYS_PRCTL, prCapBsetDrop, capVal, 0); errno != 0 {
			return fmt.Errorf("prctl(PR_CAPBSET_DROP, %s): %w", name, errno)
		}
		if debug {
			fmt.Printf("[init] dropped capability %s (%d)\n", name, capVal)
		}
	}
	return nil
}
