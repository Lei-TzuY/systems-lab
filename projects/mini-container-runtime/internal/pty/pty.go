package pty

import (
	"fmt"
	"io"
	"os"
	"os/signal"
	"syscall"
)

// Session represents an interactive terminal session config.
type Session struct {
	Stdin  io.Reader
	Stdout io.Writer
	Stderr io.Writer
	Raw    bool
	Width  uint16
	Height uint16
}

// NewSession creates an interactive PTY session handler.
func NewSession(stdin io.Reader, stdout, stderr io.Writer, tty bool) *Session {
	return &Session{
		Stdin:  stdin,
		Stdout: stdout,
		Stderr: stderr,
		Raw:    tty,
		Width:  80,
		Height: 24,
	}
}

// SetupSignalHandler captures terminal resize / interrupt signals.
func (s *Session) SetupSignalHandler(onResize func(w, h uint16)) func() {
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT)

	done := make(chan struct{})
	go func() {
		for {
			select {
			case sig := <-sigChan:
				if sig == syscall.SIGINT {
					fmt.Fprintln(s.Stderr, "\n[Interrupt received]")
				}
			case <-done:
				return
			}
		}
	}()

	return func() {
		signal.Stop(sigChan)
		close(done)
	}
}

// PipeStreams copies stdio streams between container and terminal.
func PipeStreams(in io.Reader, out io.Writer, errWriter io.Writer, inPipe io.WriteCloser, outPipe io.Reader, errPipe io.Reader) error {
	errCh := make(chan error, 2)

	go func() {
		if in != nil && inPipe != nil {
			_, _ = io.Copy(inPipe, in)
			_ = inPipe.Close()
		}
	}()

	go func() {
		if out != nil && outPipe != nil {
			_, err := io.Copy(out, outPipe)
			errCh <- err
		} else {
			errCh <- nil
		}
	}()

	go func() {
		if errWriter != nil && errPipe != nil {
			_, err := io.Copy(errWriter, errPipe)
			errCh <- err
		} else {
			errCh <- nil
		}
	}()

	var firstErr error
	for i := 0; i < 2; i++ {
		if err := <-errCh; err != nil && firstErr == nil {
			firstErr = err
		}
	}
	return firstErr
}
