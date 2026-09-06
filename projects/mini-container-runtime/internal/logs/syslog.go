package logs

import (
	"fmt"
	"time"
)

type SyslogEntry struct {
	Timestamp   time.Time `json:"timestamp"`
	ContainerID string    `json:"container_id"`
	Tag         string    `json:"tag"`
	Message     string    `json:"message"`
}

// FormatSyslogEntry formats a log line into standard RFC5424 syslog header format.
func FormatSyslogEntry(containerID string, tag string, message string) string {
	if tag == "" {
		tag = "minictl"
	}
	ts := time.Now().Format(time.RFC3339)
	return fmt.Sprintf("<14>1 %s localhost %s %s - - %s", ts, tag, containerID[:min(8, len(containerID))], message)
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
