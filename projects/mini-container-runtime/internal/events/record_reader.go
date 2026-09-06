package events

import (
	"bufio"
	"errors"
	"fmt"
	"io"
)

// maxEventRecordBytes is a hard upper bound for one durable JSON event record,
// excluding its terminating newline. It prevents a corrupted or attacker-
// controlled events.log from making queries/followers allocate without bound.
const maxEventRecordBytes = 1 << 20

func eventRecordTooLargeError() error {
	return fmt.Errorf("event record exceeds maximum size of %d bytes", maxEventRecordBytes)
}

func newEventRecordReader(r io.Reader) *bufio.Reader {
	// One extra byte lets a record exactly at the limit plus its newline fit in
	// one slice, while any larger unterminated prefix trips ErrBufferFull.
	return bufio.NewReaderSize(r, maxEventRecordBytes+1)
}

func readEventRecord(reader *bufio.Reader) ([]byte, error) {
	line, err := reader.ReadSlice('\n')
	if errors.Is(err, bufio.ErrBufferFull) {
		return nil, eventRecordTooLargeError()
	}

	payloadLen := len(line)
	if payloadLen > 0 && line[payloadLen-1] == '\n' {
		payloadLen--
	}
	if payloadLen > maxEventRecordBytes {
		return nil, eventRecordTooLargeError()
	}
	return line, err
}
