//go:build !linux

package events

import (
	"errors"
	"os"
)

type eventFollowStartupSnapshot struct {
	retained []eventLogSnapshotFile
	active   *os.File
}

func (snapshot *eventFollowStartupSnapshot) close() {
	for _, generation := range snapshot.retained {
		_ = generation.file.Close()
	}
	if snapshot.active != nil {
		_ = snapshot.active.Close()
	}
}

func openEventLogFollowStartupSnapshot(path string) (*eventFollowStartupSnapshot, error) {
	active, err := openEventLogForRead(path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return &eventFollowStartupSnapshot{}, nil
		}
		return nil, err
	}
	return &eventFollowStartupSnapshot{active: active}, nil
}

func openEventLogFollowSuccessors(path string, previous *os.File) ([]*os.File, error) {
	active, err := openEventLogForRead(path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil, nil
		}
		return nil, err
	}
	return []*os.File{active}, nil
}
