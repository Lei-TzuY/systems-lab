//go:build !linux

// internal/ns/ns_other.go
// Non-Linux build stub for namespace configuration.

package ns

import "syscall"

type Options struct {
	UserNS  bool
	HostUID int
	HostGID int
}

func BuildCloneFlags(opts Options) *syscall.SysProcAttr {
	return &syscall.SysProcAttr{}
}
