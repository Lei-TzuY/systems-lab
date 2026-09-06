package logs

import (
	"strings"
	"testing"
)

func TestFormatSyslogEntry(t *testing.T) {
	entry := FormatSyslogEntry("ctr-syslog-123456", "stdout", "Server listening on :8080")
	if !strings.Contains(entry, "<14>1") {
		t.Fatalf("Syslog entry missing RFC5424 header: %s", entry)
	}
	if !strings.Contains(entry, "ctr-sysl") {
		t.Fatalf("Syslog entry missing container short ID: %s", entry)
	}
	if !strings.Contains(entry, "Server listening on :8080") {
		t.Fatalf("Syslog entry missing log message: %s", entry)
	}
}
