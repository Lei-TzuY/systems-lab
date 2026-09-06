//go:build !linux

package events

import (
	"errors"
	"os"
)

func openEventLogSnapshotForRead(path string) ([]eventLogSnapshotFile, error) {
	file, err := openEventLogForRead(path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil, nil
		}
		return nil, err
	}
	info, err := file.Stat()
	if err != nil {
		_ = file.Close()
		return nil, err
	}
	return []eventLogSnapshotFile{{file: file, size: info.Size()}}, nil
}
