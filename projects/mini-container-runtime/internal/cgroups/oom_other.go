//go:build !linux

package cgroups

type OOMEvents struct {
	OOMCount     uint64 `json:"oom_count"`
	OOMKillCount uint64 `json:"oom_kill_count"`
}

func ReadOOMEvents(cgroupPath string) (*OOMEvents, error) {
	return &OOMEvents{}, nil
}
