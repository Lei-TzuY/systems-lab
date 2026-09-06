// internal/events/events.go
//
// Container Real-time Lifecycle Event Audit Stream (`minictl events`)
// ───────────────────────────────────────────────────────────────────
// Emits and logs container lifecycle events (create, start, exec, pause, unpause, stop, signal, die, rm).

package events

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"time"
	"unicode"

	"minicontainer/internal/state"
)

// EventType represents the category of container action.
type EventType string

const (
	EventCreate  EventType = "create"
	EventStart   EventType = "start"
	EventExec    EventType = "exec"
	EventPause   EventType = "pause"
	EventUnpause EventType = "unpause"
	EventStop    EventType = "stop"
	EventSignal  EventType = "signal"
	EventDie     EventType = "die"
	EventRemove  EventType = "destroy"
)

// Event describes a single container lifecycle event.
type Event struct {
	Timestamp   time.Time `json:"timestamp"`
	Type        EventType `json:"type"`
	ContainerID string    `json:"container_id"`
	Image       string    `json:"image,omitempty"`
	Message     string    `json:"message,omitempty"`
	// ContainerPID and ContainerPIDStartTime identify the exact init-process
	// generation to which an exec lifecycle event was admitted. PID alone is not
	// sufficient because Linux can reuse numeric process IDs after restart.
	ContainerPID          int      `json:"container_pid,omitempty"`
	ContainerPIDStartTime uint64   `json:"container_pid_start_time,omitempty"`
	Command               []string `json:"command,omitempty"`
	// ExitCode is structured terminal status for events that represent a process
	// outcome. A pointer distinguishes an actual exit code of zero from events
	// that have no exit-code semantics.
	ExitCode *int `json:"exit_code,omitempty"`
	// Error carries a machine-readable failure detail alongside the legacy human
	// Message. It is populated for failed exec admission/setup and abnormal waits.
	Error string `json:"error,omitempty"`
}

// StreamOptions controls event query and rendering without changing the durable
// append-only log schema. ContainerPrefix is intentionally a prefix selector so
// callers can use the same short IDs exposed by other minictl commands. Types is
// an OR filter; an empty slice selects every event type. Since and Until are
// inclusive absolute timestamp bounds; zero values leave that side unbounded.
type StreamOptions struct {
	Follow          bool
	JSON            bool
	ContainerPrefix string
	Types           []EventType
	Since           time.Time
	Until           time.Time
}

type eventLogSnapshotFile struct {
	file *os.File
	size int64
}

var mu sync.Mutex

// LogPath returns the path to the events append-only log file.
func LogPath() string {
	return filepath.Join(state.DefaultDir(), "events.log")
}

// Publish records a new event to the global log. Runtime-admission events are
// staged when the CLI announces intent before the operation can prove success:
// start is committed only after payload exec and exec is committed only after
// the exec payload process itself starts. Create is persisted only after the
// exact durable container record exists. A die event is persisted only when
// this process has a committed start proof for the same container.
func Publish(evtType EventType, containerID, image, message string) error {
	mu.Lock()
	defer mu.Unlock()

	if evtType == EventCreate {
		if err := validatePersistedCreate(containerID); err != nil {
			return err
		}
	}
	if evtType == EventStart || evtType == EventExec {
		if err := validateEventStagingStorage(); err != nil {
			return err
		}
		if evtType == EventStart {
			return stageStartEvent(containerID, image, message)
		}
		return stageExecEvent(containerID, image, message)
	}
	if evtType == EventDie {
		if !consumeDieProof(containerID) {
			return nil
		}
		defer finishDieProof(containerID)
	}

	evt := Event{
		Timestamp:   time.Now(),
		Type:        evtType,
		ContainerID: containerID,
		Image:       image,
		Message:     message,
	}
	return appendEventUnlocked(evt)
}

func appendEventUnlocked(evt Event) error {
	data, err := json.Marshal(evt)
	if err != nil {
		return err
	}
	if len(data) > maxEventRecordBytes {
		return fmt.Errorf("event record exceeds maximum size of %d bytes", maxEventRecordBytes)
	}

	f, err := openEventLogForAppend(LogPath())
	if err != nil {
		return err
	}
	defer f.Close()

	if _, err := fmt.Fprintln(f, string(data)); err != nil {
		return err
	}
	if err := f.Sync(); err != nil {
		return fmt.Errorf("sync event log: %w", err)
	}
	return nil
}

func formatHumanEventText(value string) string {
	if strings.IndexFunc(value, unicode.IsControl) >= 0 {
		return strconv.Quote(value)
	}
	return value
}

func FormatEvent(evt Event) string {
	shortID := evt.ContainerID
	if len(shortID) > 12 {
		shortID = shortID[:12]
	}

	var b strings.Builder
	fmt.Fprintf(&b, "%s container %s %s", evt.Timestamp.Format(time.RFC3339), evt.Type, shortID)
	if evt.ContainerPID > 0 {
		fmt.Fprintf(&b, " pid=%d", evt.ContainerPID)
	}
	if evt.ContainerPIDStartTime != 0 {
		fmt.Fprintf(&b, " pid_start=%d", evt.ContainerPIDStartTime)
	}
	if len(evt.Command) > 0 {
		if command, err := json.Marshal(evt.Command); err == nil {
			fmt.Fprintf(&b, " command=%s", command)
		}
	}
	if evt.ExitCode != nil {
		fmt.Fprintf(&b, " exit_code=%d", *evt.ExitCode)
	}
	if evt.Error != "" {
		fmt.Fprintf(&b, " error=%s", strconv.Quote(evt.Error))
	}
	if evt.Message != "" {
		fmt.Fprintf(&b, " (%s)", formatHumanEventText(evt.Message))
	}
	return b.String()
}

func eventMatchesQuery(evt Event, opts StreamOptions) bool {
	if opts.ContainerPrefix != "" && !strings.HasPrefix(evt.ContainerID, opts.ContainerPrefix) {
		return false
	}
	if !opts.Since.IsZero() && evt.Timestamp.Before(opts.Since) {
		return false
	}
	if !opts.Until.IsZero() && evt.Timestamp.After(opts.Until) {
		return false
	}
	if len(opts.Types) == 0 {
		return true
	}
	for _, eventType := range opts.Types {
		if evt.Type == eventType {
			return true
		}
	}
	return false
}

func validateStreamOptions(opts StreamOptions) error {
	if !opts.Since.IsZero() && !opts.Until.IsZero() && opts.Since.After(opts.Until) {
		return fmt.Errorf("since timestamp must not be after until timestamp")
	}
	for _, eventType := range opts.Types {
		switch eventType {
		case EventCreate, EventStart, EventExec, EventPause, EventUnpause, EventStop, EventSignal, EventDie, EventRemove, EventExecExit, EventExecFailed:
		default:
			if eventType == "" {
				return fmt.Errorf("event type filter must not be empty")
			}
			return fmt.Errorf("unknown event type filter %q", eventType)
		}
	}
	return nil
}

func validateEventRecord(evt Event) error {
	if evt.Timestamp.IsZero() {
		return fmt.Errorf("missing timestamp")
	}
	if evt.ContainerID == "" {
		return fmt.Errorf("missing container_id")
	}
	switch evt.Type {
	case EventCreate, EventStart, EventExec, EventPause, EventUnpause, EventStop, EventSignal, EventDie, EventRemove, EventExecExit, EventExecFailed:
	default:
		if evt.Type == "" {
			return fmt.Errorf("missing type")
		}
		return fmt.Errorf("unknown type %q", evt.Type)
	}
	if evt.ContainerPID < 0 {
		return fmt.Errorf("invalid container_pid %d", evt.ContainerPID)
	}
	if (evt.ContainerPID > 0) != (evt.ContainerPIDStartTime != 0) {
		return fmt.Errorf("incomplete container process generation")
	}
	return nil
}

func writeQueriedEvent(w io.Writer, evt Event, jsonOutput bool) error {
	if jsonOutput {
		data, err := json.Marshal(evt)
		if err != nil {
			return fmt.Errorf("encode event stream: %w", err)
		}
		if _, err := fmt.Fprintln(w, string(data)); err != nil {
			return fmt.Errorf("write event stream: %w", err)
		}
		return nil
	}
	if _, err := fmt.Fprintln(w, FormatEvent(evt)); err != nil {
		return fmt.Errorf("write event stream: %w", err)
	}
	return nil
}

func decodeEventRecord(line []byte) (Event, error) {
	if err := rejectDuplicateTopLevelFields(line); err != nil {
		return Event{}, fmt.Errorf("decode event log: %w", err)
	}
	var evt Event
	if err := json.Unmarshal(line, &evt); err != nil {
		return Event{}, fmt.Errorf("decode event log: %w", err)
	}
	if err := validateEventRecord(evt); err != nil {
		return Event{}, fmt.Errorf("validate event log: %w", err)
	}
	return evt, nil
}

func streamEventLogWithOptions(r io.Reader, opts StreamOptions, w io.Writer) error {
	reader := newEventRecordReader(r)
	for {
		line, err := readEventRecord(reader)
		if len(line) > 0 {
			if opts.Follow && err == io.EOF {
				reader = newEventRecordReader(io.MultiReader(bytes.NewReader(line), reader))
			} else {
				evt, decodeErr := decodeEventRecord(line)
				if decodeErr != nil {
					// At EOF, syntactically incomplete JSON is the recoverable torn-tail
					// case. A complete JSON object that fails semantic or ambiguity
					// validation must still fail closed even without a final newline.
					if err != io.EOF || json.Valid(line) {
						return decodeErr
					}
				} else if eventMatchesQuery(evt, opts) {
					if writeErr := writeQueriedEvent(w, evt, opts.JSON); writeErr != nil {
						return writeErr
					}
				}
			}
		}

		if err == nil {
			continue
		}
		if err != io.EOF {
			return fmt.Errorf("read event log: %w", err)
		}
		if !opts.Follow {
			return nil
		}
		time.Sleep(200 * time.Millisecond)
	}
}

func streamEventLog(r io.Reader, follow bool, w io.Writer) error {
	return streamEventLogWithOptions(r, StreamOptions{Follow: follow}, w)
}

type eventLogOpenFunc func(string) (*os.File, error)

func openEventLogForStreamWith(logFile string, follow bool, open eventLogOpenFunc, wait func()) (*os.File, error) {
	for {
		f, err := open(logFile)
		if err == nil {
			return f, nil
		}
		if !os.IsNotExist(err) || !follow {
			return nil, err
		}
		wait()
	}
}

func openEventLogForStream(logFile string, follow bool) (*os.File, error) {
	return openEventLogForStreamWith(logFile, follow, openEventLogForRead, func() {
		time.Sleep(200 * time.Millisecond)
	})
}

func streamHistoricalEventLogs(logFile string, opts StreamOptions, w io.Writer) error {
	snapshot, err := openEventLogSnapshotForRead(logFile)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return err
	}
	defer func() {
		for _, generation := range snapshot {
			_ = generation.file.Close()
		}
	}()

	for _, generation := range snapshot {
		if err := streamEventLogWithOptions(io.LimitReader(generation.file, generation.size), opts, w); err != nil {
			return err
		}
	}
	return nil
}

func StreamEventsWithOptions(opts StreamOptions, w io.Writer) error {
	if err := validateStreamOptions(opts); err != nil {
		return fmt.Errorf("invalid event stream options: %w", err)
	}

	logFile := LogPath()
	if opts.Follow {
		return followEventLogFile(logFile, opts, w)
	}
	return streamHistoricalEventLogs(logFile, opts, w)
}

func StreamEvents(follow bool, w io.Writer) error {
	return StreamEventsWithOptions(StreamOptions{Follow: follow}, w)
}
