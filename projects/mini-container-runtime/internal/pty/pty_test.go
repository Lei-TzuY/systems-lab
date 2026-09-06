package pty

import (
	"bytes"
	"io"
	"testing"
)

func TestPTYSessionAndPipes(t *testing.T) {
	var inBuf bytes.Buffer
	var outBuf bytes.Buffer
	var errBuf bytes.Buffer

	inBuf.WriteString("echo hello\n")

	sess := NewSession(&inBuf, &outBuf, &errBuf, true)
	if !sess.Raw || sess.Width != 80 || sess.Height != 24 {
		t.Fatalf("NewSession returned invalid settings: %+v", sess)
	}

	rOut, wOut := io.Pipe()
	rIn, wIn := io.Pipe()

	go func() {
		data := make([]byte, 100)
		n, _ := rIn.Read(data)
		wOut.Write(data[:n])
		wOut.Close()
	}()

	err := PipeStreams(&inBuf, &outBuf, &errBuf, wIn, rOut, nil)
	if err != nil {
		t.Fatalf("PipeStreams error: %v", err)
	}

	if outBuf.String() != "echo hello\n" {
		t.Fatalf("Piped output = %q, want 'echo hello\\n'", outBuf.String())
	}
}
