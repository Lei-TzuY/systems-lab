//go:build !linux

package container

import "fmt"

func ProcessStartTime(pid int) (uint64, error) {
	return 0, fmt.Errorf("process start identity requires Linux")
}

func ProcessIdentityMatches(pid int, expectedStartTime uint64) (bool, error) {
	return false, nil
}
