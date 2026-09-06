package events

import (
	"fmt"
	"io"
	"os"
)

func closeEventFollowFiles(files []*os.File) {
	for _, file := range files {
		_ = file.Close()
	}
}

// followEventLogAttachment drains one already-open generation and, whenever it
// reaches a generation boundary, snapshots and drains all retained successors
// before attaching to the newest active generation. Successor descriptors stay
// open while older generations are drained, so later renames cannot create a
// gap between discovery and handoff.
func followEventLogAttachment(initial *os.File, logFile string, opts StreamOptions, w io.Writer) (bool, error) {
	current := initial
	var queued []*os.File

	for {
		reopen, err := followOpenEventLog(current, logFile, opts, w)
		if err != nil {
			_ = current.Close()
			closeEventFollowFiles(queued)
			return false, err
		}
		if !reopen {
			closeErr := current.Close()
			closeEventFollowFiles(queued)
			if closeErr != nil {
				return false, fmt.Errorf("close event log: %w", closeErr)
			}
			return false, nil
		}

		if len(queued) == 0 {
			successors, successorErr := openEventLogFollowSuccessors(logFile, current)
			if successorErr != nil {
				_ = current.Close()
				return false, successorErr
			}
			queued = successors
		}

		closeErr := current.Close()
		if closeErr != nil {
			closeEventFollowFiles(queued)
			return false, fmt.Errorf("close event log: %w", closeErr)
		}
		if len(queued) == 0 {
			return true, nil
		}
		current = queued[0]
		queued = queued[1:]
	}
}
