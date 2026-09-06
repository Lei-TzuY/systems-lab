//go:build linux

package container

import (
	"errors"
	"os/exec"
)

// RunPayloadExitCode returns the regular process exit status only when the
// entire error tree represents exactly one payload exit and nothing else.
// Runtime-control failures, lifecycle/cleanup failures joined with a payload
// exit, signals, unrelated errors, and multi-exit trees fail closed so the CLI
// can use its generic runtime status.
func RunPayloadExitCode(err error) (int, bool) {
	if err == nil || isRuntimeControlError(err) {
		return 0, false
	}
	code, count, pure := purePayloadExitCode(err)
	if !pure || count != 1 || code <= 0 || code > 255 {
		return 0, false
	}
	return code, true
}

func purePayloadExitCode(err error) (code int, count int, pure bool) {
	if err == nil {
		return 0, 0, true
	}
	if exitErr, ok := err.(*exec.ExitError); ok {
		return exitErr.ExitCode(), 1, true
	}

	if joined, ok := err.(interface{ Unwrap() []error }); ok {
		var payloadCode int
		payloadCount := 0
		for _, child := range joined.Unwrap() {
			childCode, childCount, childPure := purePayloadExitCode(child)
			if !childPure {
				return 0, 0, false
			}
			if childCount == 0 {
				continue
			}
			if payloadCount > 0 && childCode != payloadCode {
				return 0, 0, false
			}
			payloadCode = childCode
			payloadCount += childCount
		}
		return payloadCode, payloadCount, true
	}

	if wrapped := errors.Unwrap(err); wrapped != nil {
		return purePayloadExitCode(wrapped)
	}
	return 0, 0, false
}
