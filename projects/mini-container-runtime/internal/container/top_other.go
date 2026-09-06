//go:build !linux

// internal/container/top_other.go
// Non-Linux build stub for container process listing.

package container

import "fmt"

type ProcessInfo struct {
	PID   int
	PPID  int
	Name  string
	State string
}

func GetContainerProcesses(containerPID int) ([]ProcessInfo, error) {
	return nil, fmt.Errorf("minictl top requires Linux /proc filesystem")
}
