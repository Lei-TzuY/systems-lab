// Package imagestore provides OCI image configuration inspection utilities.
// This file implements a StopSignal auditor that resolves OCI Image Config StopSignal
// definitions into canonical Linux signal names and numeric codes.

package imagestore

import (
	"encoding/json"
	"fmt"
	"strconv"
	"strings"
)

// StopSignalSummary represents evaluated stop signal and graceful timeout parameters.
type StopSignalSummary struct {
	DeclaredSignal    string
	CanonicalSignal   string
	SignalNumber      int
	IsGraceful        bool // false for uncatchable stop/kill signals
	DefaultTimeoutSec int
}

const (
	maxLinuxSignalNumber = 64
	linuxSIGRTMIN         = 34
	linuxSIGRTMAX         = 64
)

// Linux signal numbering used by the runtime. Aliases are accepted on input,
// while canonicalSignalByNumber keeps numeric declarations deterministic.
var signalNumberByName = map[string]int{
	"SIGHUP": 1, "SIGINT": 2, "SIGQUIT": 3, "SIGILL": 4,
	"SIGTRAP": 5, "SIGABRT": 6, "SIGIOT": 6, "SIGBUS": 7,
	"SIGFPE": 8, "SIGKILL": 9, "SIGUSR1": 10, "SIGSEGV": 11,
	"SIGUSR2": 12, "SIGPIPE": 13, "SIGALRM": 14, "SIGTERM": 15,
	"SIGSTKFLT": 16, "SIGCHLD": 17, "SIGCLD": 17, "SIGCONT": 18,
	"SIGSTOP": 19, "SIGTSTP": 20, "SIGTTIN": 21, "SIGTTOU": 22,
	"SIGURG": 23, "SIGXCPU": 24, "SIGXFSZ": 25, "SIGVTALRM": 26,
	"SIGPROF": 27, "SIGWINCH": 28, "SIGIO": 29, "SIGPOLL": 29,
	"SIGPWR": 30, "SIGSYS": 31, "SIGUNUSED": 31,
}

var canonicalSignalByNumber = map[int]string{
	1: "SIGHUP", 2: "SIGINT", 3: "SIGQUIT", 4: "SIGILL",
	5: "SIGTRAP", 6: "SIGABRT", 7: "SIGBUS", 8: "SIGFPE",
	9: "SIGKILL", 10: "SIGUSR1", 11: "SIGSEGV", 12: "SIGUSR2",
	13: "SIGPIPE", 14: "SIGALRM", 15: "SIGTERM", 16: "SIGSTKFLT",
	17: "SIGCHLD", 18: "SIGCONT", 19: "SIGSTOP", 20: "SIGTSTP",
	21: "SIGTTIN", 22: "SIGTTOU", 23: "SIGURG", 24: "SIGXCPU",
	25: "SIGXFSZ", 26: "SIGVTALRM", 27: "SIGPROF", 28: "SIGWINCH",
	29: "SIGIO", 30: "SIGPWR", 31: "SIGSYS",
}

// EvaluateStopSignal parses image config StopSignal and returns structured signal data.
func EvaluateStopSignal(configJSON []byte) (StopSignalSummary, error) {
	var cfg struct {
		Config struct {
			StopSignal string `json:"StopSignal,omitempty"`
		} `json:"config"`
	}
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return StopSignalSummary{}, fmt.Errorf("parse image config for stop signal: %w", err)
	}

	raw := strings.TrimSpace(cfg.Config.StopSignal)
	if raw == "" {
		return stopSignalSummary(raw, "SIGTERM", 15), nil
	}

	upper := strings.ToUpper(raw)
	if num, err := strconv.Atoi(upper); err == nil {
		if num <= 0 || num > maxLinuxSignalNumber {
			return StopSignalSummary{}, fmt.Errorf("invalid stop signal number %d: expected 1-%d", num, maxLinuxSignalNumber)
		}
		canonical := canonicalSignalByNumber[num]
		if canonical == "" {
			canonical = fmt.Sprintf("SIG_%d", num)
		}
		return stopSignalSummary(raw, canonical, num), nil
	}

	if !strings.HasPrefix(upper, "SIG") {
		upper = "SIG" + upper
	}

	if canonical, num, matched, err := parseRealtimeSignal(upper); matched {
		if err != nil {
			return StopSignalSummary{}, err
		}
		return stopSignalSummary(raw, canonical, num), nil
	}

	if num, ok := signalNumberByName[upper]; ok {
		canonical := canonicalSignalByNumber[num]
		if canonical == "" {
			canonical = upper
		}
		return stopSignalSummary(raw, canonical, num), nil
	}

	return StopSignalSummary{}, fmt.Errorf("unknown or unsupported stop signal %q", raw)
}

func stopSignalSummary(declared, canonical string, num int) StopSignalSummary {
	graceful := num != signalNumberByName["SIGKILL"] && num != signalNumberByName["SIGSTOP"]
	timeout := 10
	if !graceful {
		timeout = 0
	}
	return StopSignalSummary{
		DeclaredSignal:    declared,
		CanonicalSignal:   canonical,
		SignalNumber:      num,
		IsGraceful:        graceful,
		DefaultTimeoutSec: timeout,
	}
}

// parseRealtimeSignal accepts OCI-style SIGRTMIN+N and the symmetric
// SIGRTMAX-N form. It also accepts the existing no-SIG-prefix convenience
// after EvaluateStopSignal normalizes the prefix.
func parseRealtimeSignal(signal string) (canonical string, num int, matched bool, err error) {
	switch signal {
	case "SIGRTMIN":
		return "SIGRTMIN", linuxSIGRTMIN, true, nil
	case "SIGRTMAX":
		return "SIGRTMAX", linuxSIGRTMAX, true, nil
	}

	if strings.HasPrefix(signal, "SIGRTMIN+") {
		offsetText := strings.TrimPrefix(signal, "SIGRTMIN+")
		offset, parseErr := strconv.Atoi(offsetText)
		if parseErr != nil || offset < 0 || linuxSIGRTMIN+offset > linuxSIGRTMAX {
			return "", 0, true, fmt.Errorf("invalid realtime stop signal %q", signal)
		}
		return fmt.Sprintf("SIGRTMIN+%d", offset), linuxSIGRTMIN + offset, true, nil
	}

	if strings.HasPrefix(signal, "SIGRTMAX-") {
		offsetText := strings.TrimPrefix(signal, "SIGRTMAX-")
		offset, parseErr := strconv.Atoi(offsetText)
		if parseErr != nil || offset < 0 || linuxSIGRTMAX-offset < linuxSIGRTMIN {
			return "", 0, true, fmt.Errorf("invalid realtime stop signal %q", signal)
		}
		return fmt.Sprintf("SIGRTMAX-%d", offset), linuxSIGRTMAX - offset, true, nil
	}

	return "", 0, false, nil
}

// FormatStopSignal returns a human-readable stop signal summary.
func FormatStopSignal(configJSON []byte) string {
	summary, err := EvaluateStopSignal(configJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}

	return fmt.Sprintf("Stop Signal: %s (num: %d, graceful: %t, timeout: %ds)",
		summary.CanonicalSignal, summary.SignalNumber, summary.IsGraceful, summary.DefaultTimeoutSec)
}
