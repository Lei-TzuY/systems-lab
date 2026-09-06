//go:build !linux

package dns

import "fmt"

type registrarIdentity struct {
	PID       int
	StartTime uint64
}

func currentRegistrarIdentity() (registrarIdentity, error) {
	return registrarIdentity{}, fmt.Errorf("DNS registrar process identity requires Linux")
}

func registrarGenerationAlive(pid int, startTime uint64) (bool, error) {
	return false, fmt.Errorf("DNS registrar process identity requires Linux")
}
