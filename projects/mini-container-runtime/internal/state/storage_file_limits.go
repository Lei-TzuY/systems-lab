package state

import (
	"fmt"
	"io"
	"os"
)

const maxStateFileBytes int64 = 4 << 20 // 4 MiB

func validateStateFileWrite(data []byte, label string) error {
	if int64(len(data)) > maxStateFileBytes {
		return fmt.Errorf("%s exceeds %d-byte size limit", label, maxStateFileBytes)
	}
	return nil
}

func readBoundedStateFile(file *os.File, observedSize int64, label string) ([]byte, error) {
	if observedSize > maxStateFileBytes {
		return nil, fmt.Errorf("%s exceeds %d-byte size limit", label, maxStateFileBytes)
	}

	data, err := io.ReadAll(io.LimitReader(file, maxStateFileBytes+1))
	if err != nil {
		return nil, fmt.Errorf("read %s: %w", label, err)
	}
	if int64(len(data)) > maxStateFileBytes {
		return nil, fmt.Errorf("%s exceeds %d-byte size limit", label, maxStateFileBytes)
	}
	return data, nil
}
