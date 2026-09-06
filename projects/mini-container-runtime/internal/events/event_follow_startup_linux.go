//go:build linux

package events

import (
	"errors"
	"fmt"
	"os"

	"golang.org/x/sys/unix"
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

// openEventLogFollowStartupSnapshot captures all retained generations and the
// active descriptor under the same writer lock. Holding these descriptors
// across the handoff makes concurrent renames safe and preserves oldest-first
// ordering across the bounded retention window.
func openEventLogFollowStartupSnapshot(path string) (*eventFollowStartupSnapshot, error) {
	lockPath := path + ".lock"
	lockFile, err := openEventLog(lockPath, unix.O_RDWR|unix.O_CREAT, 0o600)
	if err != nil {
		return nil, fmt.Errorf("open event log follow snapshot lock: %w", err)
	}
	defer lockFile.Close()

	if err := unix.Flock(int(lockFile.Fd()), unix.LOCK_SH); err != nil {
		return nil, fmt.Errorf("lock event log follow snapshot: %w", err)
	}
	defer unix.Flock(int(lockFile.Fd()), unix.LOCK_UN)
	if err := verifyLockedEventPath(lockFile, lockPath); err != nil {
		return nil, err
	}

	snapshot := &eventFollowStartupSnapshot{}
	for _, retainedPath := range []string{path + ".2", path + ".1"} {
		retained, err := openEventLogForRead(retainedPath)
		if err == nil {
			if err := verifyHeldEventPath(retained, retainedPath, "event log follow retained snapshot"); err != nil {
				_ = retained.Close()
				snapshot.close()
				return nil, err
			}
			info, err := retained.Stat()
			if err != nil {
				_ = retained.Close()
				snapshot.close()
				return nil, fmt.Errorf("stat retained event log follow snapshot: %w", err)
			}
			snapshot.retained = append(snapshot.retained, eventLogSnapshotFile{file: retained, size: info.Size()})
		} else if !errors.Is(err, os.ErrNotExist) {
			snapshot.close()
			return nil, err
		}
	}

	active, err := openEventLogForRead(path)
	if err == nil {
		if err := verifyHeldEventPath(active, path, "event log follow active snapshot"); err != nil {
			_ = active.Close()
			snapshot.close()
			return nil, err
		}
		snapshot.active = active
	} else if !errors.Is(err, os.ErrNotExist) {
		snapshot.close()
		return nil, err
	}
	return snapshot, nil
}

type eventFollowGeneration struct {
	file   *os.File
	info   os.FileInfo
	active bool
}

func closeEventFollowGenerations(generations []eventFollowGeneration) {
	for _, generation := range generations {
		_ = generation.file.Close()
	}
}

// openEventLogFollowSuccessors snapshots the retained window plus active path
// under the writer lock and returns every generation after previous. This lets a
// live follower catch up across two rapid rotations without silently jumping to
// the newest active file. If previous has already fallen out of the retention
// window while newer generations exist, report an explicit gap instead.
func openEventLogFollowSuccessors(path string, previous *os.File) ([]*os.File, error) {
	previousInfo, err := previous.Stat()
	if err != nil {
		return nil, fmt.Errorf("stat previous event log generation: %w", err)
	}

	lockPath := path + ".lock"
	lockFile, err := openEventLog(lockPath, unix.O_RDWR|unix.O_CREAT, 0o600)
	if err != nil {
		return nil, fmt.Errorf("open event log successor snapshot lock: %w", err)
	}
	defer lockFile.Close()
	if err := unix.Flock(int(lockFile.Fd()), unix.LOCK_SH); err != nil {
		return nil, fmt.Errorf("lock event log successor snapshot: %w", err)
	}
	defer unix.Flock(int(lockFile.Fd()), unix.LOCK_UN)
	if err := verifyLockedEventPath(lockFile, lockPath); err != nil {
		return nil, err
	}

	paths := []string{path + ".2", path + ".1", path}
	generations := make([]eventFollowGeneration, 0, len(paths))
	for _, candidatePath := range paths {
		candidate, openErr := openEventLogForRead(candidatePath)
		if openErr != nil {
			if errors.Is(openErr, os.ErrNotExist) {
				continue
			}
			closeEventFollowGenerations(generations)
			return nil, openErr
		}
		if err := verifyHeldEventPath(candidate, candidatePath, "event log follow successor snapshot"); err != nil {
			_ = candidate.Close()
			closeEventFollowGenerations(generations)
			return nil, err
		}
		info, statErr := candidate.Stat()
		if statErr != nil {
			_ = candidate.Close()
			closeEventFollowGenerations(generations)
			return nil, fmt.Errorf("stat event log successor snapshot: %w", statErr)
		}
		generations = append(generations, eventFollowGeneration{file: candidate, info: info, active: candidatePath == path})
	}

	if len(generations) == 0 {
		return nil, fmt.Errorf("event follow generation gap: previous generation disappeared with no retained successor")
	}

	match := -1
	for i, generation := range generations {
		if os.SameFile(previousInfo, generation.info) {
			match = i
			break
		}
	}
	if match < 0 {
		closeEventFollowGenerations(generations)
		return nil, fmt.Errorf("event follow generation gap: previous generation is no longer retained")
	}

	// Same inode at the active pathname means inspectEventLogGeneration detected
	// an in-place reset (for example copytruncate). Re-read that active generation
	// from offset zero rather than treating it as already consumed.
	if generations[match].active {
		result := []*os.File{generations[match].file}
		closeEventFollowGenerations(generations[:match])
		return result, nil
	}

	closeEventFollowGenerations(generations[:match+1])
	result := make([]*os.File, 0, len(generations)-match-1)
	for _, generation := range generations[match+1:] {
		result = append(result, generation.file)
	}
	return result, nil
}
