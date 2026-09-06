package logs

import (
	"fmt"
	"time"
)

// FormatLogToSyslog formats raw log line into a Syslog RFC5424 compliant string.
func FormatLogToSyslog(containerID, rawLine string) string {
	ts := time.Now().UTC().Format(time.RFC3339)
	return fmt.Sprintf("<14>1 %s localhost minictl %s - - %s\n", ts, containerID, rawLine)
}
