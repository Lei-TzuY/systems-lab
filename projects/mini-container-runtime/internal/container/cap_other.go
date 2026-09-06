//go:build !linux

// internal/container/cap_other.go
// Non-Linux build stub for Linux capability management.

package container

import "fmt"

func DropCapabilities(_ []string, _ bool) error {
	return fmt.Errorf("linux capabilities management requires Linux")
}
