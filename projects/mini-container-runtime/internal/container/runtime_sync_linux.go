//go:build linux

package container

import (
	"fmt"
	"io"
	"os"
)

const runtimeReadyByte byte = 0xa5

// releaseBlockedChild commits parent-side runtime setup by writing one explicit
// readiness byte to the child's private sync pipe. Closing the pipe is not a
// readiness signal: an unexpected parent exit also closes the writer, so the
// child must never treat EOF as permission to continue.
//
// Successful delivery means the child may proceed with container initialization;
// it does not prove that the payload image has successfully exec'd. Lifecycle
// start observability is therefore committed later at the init-status CLOEXEC
// boundary in awaitPayloadExec.
func releaseBlockedChild(writePipe *os.File) error {
	if writePipe == nil {
		return fmt.Errorf("runtime sync writer is nil")
	}

	n, err := writePipe.Write([]byte{runtimeReadyByte})
	if n == 1 {
		_ = writePipe.Close()
		return nil
	}
	_ = writePipe.Close()
	if err != nil {
		return fmt.Errorf("write runtime ready byte: %w", err)
	}
	return fmt.Errorf("write runtime ready byte: %w", io.ErrShortWrite)
}

// awaitParentReady blocks the re-executed child until the parent explicitly
// commits runtime setup. EOF means the parent disappeared or closed the pipe
// without committing, and therefore fails closed.
func awaitParentReady(readPipe *os.File) error {
	if readPipe == nil {
		return fmt.Errorf("runtime sync reader is nil")
	}
	defer readPipe.Close()

	var ready [1]byte
	if _, err := io.ReadFull(readPipe, ready[:]); err != nil {
		return fmt.Errorf("await parent runtime readiness: %w", err)
	}
	if ready[0] != runtimeReadyByte {
		return fmt.Errorf("invalid runtime ready byte 0x%02x", ready[0])
	}
	return nil
}
