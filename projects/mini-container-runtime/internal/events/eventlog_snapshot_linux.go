//go:build linux

package events

import (
	"errors"
	"fmt"
	"os"

	"golang.org/x/sys/unix"
)

func openEventLogSnapshotForRead(path string) ([]eventLogSnapshotFile, error) {
	lockPath := path + ".lock"
	lockFile, err := openEventLog(lockPath, unix.O_RDWR|unix.O_CREAT, 0o600)
	if err != nil {
		return nil, fmt.Errorf("open event log snapshot lock: %w", err)
	}
	defer lockFile.Close()

	if err := unix.Flock(int(lockFile.Fd()), unix.LOCK_SH); err != nil {
		return nil, fmt.Errorf("lock event log snapshot: %w", err)
	}
	defer unix.Flock(int(lockFile.Fd()), unix.LOCK_UN)
	if err := verifyLockedEventPath(lockFile, lockPath); err != nil {
		return nil, err
	}

	paths := []string{path + ".2", path + ".1", path}
	snapshot := make([]eventLogSnapshotFile, 0, len(paths))
	closeSnapshot := func() {
		for _, generation := range snapshot {
			_ = generation.file.Close()
		}
	}

	for _, generationPath := range paths {
		file, err := openEventLogForRead(generationPath)
		if err != nil {
			if errors.Is(err, os.ErrNotExist) {
				continue
			}
			closeSnapshot()
			return nil, err
		}
		if err := verifyHeldEventPath(file, generationPath, "event log snapshot"); err != nil {
			_ = file.Close()
			closeSnapshot()
			return nil, err
		}
		info, err := file.Stat()
		if err != nil {
			_ = file.Close()
			closeSnapshot()
			return nil, fmt.Errorf("stat event log snapshot: %w", err)
		}
		snapshot = append(snapshot, eventLogSnapshotFile{file: file, size: info.Size()})
	}
	return snapshot, nil
}
