package logs

import (
	"encoding/json"
	"time"
)

type LogEntryJSON struct {
	ContainerID string `json:"containerId"`
	Stream      string `json:"stream"`
	Timestamp   string `json:"timestamp"`
	Message     string `json:"message"`
}

// FormatLogToJSON formats raw stdio log line into a JSON string.
func FormatLogToJSON(containerID, stream, rawLine string) (string, error) {
	entry := LogEntryJSON{
		ContainerID: containerID,
		Stream:      stream,
		Timestamp:   time.Now().UTC().Format(time.RFC3339),
		Message:     rawLine,
	}

	data, err := json.Marshal(entry)
	if err != nil {
		return "", err
	}

	return string(data), nil
}
