//go:build linux

package container

import (
	"fmt"
	"strconv"
	"strings"
	"syscall"
)

// ParseSignal converts string signal name or integer to syscall.Signal.
func ParseSignal(sigStr string) (syscall.Signal, error) {
	sigStr = strings.TrimSpace(strings.ToUpper(sigStr))
	sigStr = strings.TrimPrefix(sigStr, "SIG")

	switch sigStr {
	case "KILL", "9":
		return syscall.SIGKILL, nil
	case "TERM", "15":
		return syscall.SIGTERM, nil
	case "HUP", "1":
		return syscall.SIGHUP, nil
	case "INT", "2":
		return syscall.SIGINT, nil
	case "QUIT", "3":
		return syscall.SIGQUIT, nil
	case "USR1", "10":
		return syscall.SIGUSR1, nil
	case "USR2", "12":
		return syscall.SIGUSR2, nil
	case "STOP", "19":
		return syscall.SIGSTOP, nil
	case "CONT", "18":
		return syscall.SIGCONT, nil
	}

	if num, err := strconv.Atoi(sigStr); err == nil && num > 0 && num < 64 {
		return syscall.Signal(num), nil
	}

	return 0, fmt.Errorf("unknown or unsupported signal %q", sigStr)
}
