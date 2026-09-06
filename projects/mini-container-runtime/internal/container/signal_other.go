//go:build !linux

package container

import (
	"fmt"
	"strconv"
	"strings"
	"syscall"
)

// ParseSignal converts string signal name or integer to syscall.Signal for non-Linux OS.
func ParseSignal(sigStr string) (syscall.Signal, error) {
	sigStr = strings.TrimSpace(strings.ToUpper(sigStr))
	sigStr = strings.TrimPrefix(sigStr, "SIG")

	switch sigStr {
	case "KILL", "9":
		return syscall.SIGKILL, nil
	case "TERM", "15":
		return syscall.SIGTERM, nil
	case "INT", "2":
		return syscall.SIGINT, nil
	}

	if num, err := strconv.Atoi(sigStr); err == nil && num > 0 && num < 64 {
		return syscall.Signal(num), nil
	}

	return 0, fmt.Errorf("unknown or unsupported signal %q on host OS", sigStr)
}
