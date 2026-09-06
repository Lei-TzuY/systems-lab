package logs

import (
	"strings"
	"testing"
)

func TestFormatLogToSyslog(t *testing.T) {
	syslogStr := FormatLogToSyslog("c456", "service started")
	if !strings.HasPrefix(syslogStr, "<14>1 ") || !strings.Contains(syslogStr, "minictl c456") || !strings.Contains(syslogStr, "service started") {
		t.Fatalf("FormatLogToSyslog = %s, want Syslog RFC5424 format", syslogStr)
	}
}
