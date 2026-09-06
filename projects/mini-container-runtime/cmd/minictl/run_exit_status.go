package main

import "minicontainer/internal/container"

func runCommandExitCode(runErr error) int {
	if code, ok := container.RunPayloadExitCode(runErr); ok {
		return code
	}
	return 1
}
