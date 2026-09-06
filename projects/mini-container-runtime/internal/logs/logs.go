// internal/logs/logs.go
//
// Container Log Storage & Retrieval
// ─────────────────────────────────
// When a container is launched in background or attached mode, its stdout and
// stderr streams can be tee'd or written to a log file under:
//
//   ~/.minicontainer/containers/<id>.log
//
// This file implements log writing, tailing, and log following (`minictl logs -f`).

package logs

import (
	"bufio"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"time"

	"minicontainer/internal/state"
)

// LogFilePath returns the canonical path to a container's log file.
func LogFilePath(containerID string) string {
	return filepath.Join(state.DefaultDir(), "containers", containerID+".log")
}

func validateContainerID(containerID string) error {
	if strings.TrimSpace(containerID) == "" {
		return fmt.Errorf("container ID cannot be empty")
	}
	if containerID == "." || containerID == ".." || strings.ContainsAny(containerID, "/\\:\x00") {
		return fmt.Errorf("invalid container ID %q", containerID)
	}
	return nil
}

func shortContainerID(containerID string) string {
	if len(containerID) > 8 {
		return containerID[:8]
	}
	return containerID
}

// CreateLogFile creates or opens the container's log file for writing.
func CreateLogFile(containerID string) (*os.File, error) {
	if err := validateContainerID(containerID); err != nil {
		return nil, err
	}
	return openContainerLogForAppend(LogFilePath(containerID))
}

// PrintLogs prints the contents of the container's log file.
// If tail > 0, only the last `tail` lines are printed.
// If follow is true, it continuously streams new lines until interrupted.
func PrintLogs(containerID string, tail int, follow bool, out io.Writer) error {
	if err := validateContainerID(containerID); err != nil {
		return err
	}
	path := LogFilePath(containerID)
	f, err := openContainerLogForRead(path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return fmt.Errorf("no logs found for container %s", shortContainerID(containerID))
		}
		return fmt.Errorf("open log file: %w", err)
	}
	defer f.Close()

	if tail > 0 {
		lines, err := readLastNLines(f, tail)
		if err != nil {
			return fmt.Errorf("read last lines: %w", err)
		}
		for _, l := range lines {
			fmt.Fprintln(out, l)
		}
	} else {
		if _, err := io.Copy(out, f); err != nil {
			return err
		}
	}

	if !follow {
		return nil
	}

	// Follow mode: poll file for new content.
	reader := bufio.NewReader(f)
	for {
		line, err := reader.ReadString('\n')
		if err == nil {
			fmt.Fprint(out, line)
			continue
		}
		if err == io.EOF {
			time.Sleep(200 * time.Millisecond)
			continue
		}
		return err
	}
}

func readLastNLines(r io.ReadSeeker, n int) ([]string, error) {
	var lines []string
	scanner := bufio.NewScanner(r)
	for scanner.Scan() {
		lines = append(lines, scanner.Text())
	}
	if err := scanner.Err(); err != nil {
		return nil, err
	}
	if len(lines) > n {
		lines = lines[len(lines)-n:]
	}
	return lines, nil
}
