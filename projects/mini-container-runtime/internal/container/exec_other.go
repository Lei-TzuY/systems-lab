//go:build !linux

// internal/container/exec_other.go — non-Linux stub.

package container

import "fmt"

// ExecConfig is the non-Linux placeholder.
type ExecConfig struct {
	ContainerPID int
	RootFS       string
	Command      []string
	Debug        bool
}

func Exec(_ ExecConfig) error {
	return fmt.Errorf("exec requires Linux (setns syscall not available on this OS)")
}

func ExecInit(_ int, _ string, _ []string, _ bool) error {
	return fmt.Errorf("exec init requires Linux")
}

func IsRunning(_ int) bool    { return false }
func ContainerCwd(_ int) string { return "/" }
