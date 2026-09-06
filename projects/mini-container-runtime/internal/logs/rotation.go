package logs

import (
	"errors"
	"fmt"
	"io"
	"os"
)

const rotationCopyBufferSize = 64 * 1024

// RotateLogFile retains the newest maxBytes of an existing regular log file.
// Rotation operates on one hardened read/write file descriptor so pathname
// replacement cannot redirect the read and write phases to different files.
// The data is shifted in-place to preserve the inode used by active appenders.
func RotateLogFile(logPath string, maxBytes int64) error {
	if maxBytes <= 0 {
		return nil
	}

	f, err := openContainerLogForRotate(logPath)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil
		}
		return fmt.Errorf("open log file for rotation: %w", err)
	}
	defer f.Close()

	fi, err := f.Stat()
	if err != nil {
		return fmt.Errorf("stat log file: %w", err)
	}
	if fi.Size() <= maxBytes {
		return nil
	}

	start := fi.Size() - maxBytes
	buf := make([]byte, rotationCopyBufferSize)
	var copied int64
	for copied < maxBytes {
		want := int64(len(buf))
		if remaining := maxBytes - copied; remaining < want {
			want = remaining
		}

		n, readErr := f.ReadAt(buf[:int(want)], start+copied)
		if readErr != nil && !errors.Is(readErr, io.EOF) {
			return fmt.Errorf("read log tail: %w", readErr)
		}
		if int64(n) != want {
			return fmt.Errorf("read log tail: %w", io.ErrUnexpectedEOF)
		}
		if _, err := f.WriteAt(buf[:n], copied); err != nil {
			return fmt.Errorf("rewrite log tail: %w", err)
		}
		copied += int64(n)
	}

	if err := f.Truncate(maxBytes); err != nil {
		return fmt.Errorf("truncate rotated log: %w", err)
	}
	if err := f.Sync(); err != nil {
		return fmt.Errorf("sync rotated log: %w", err)
	}
	return nil
}
