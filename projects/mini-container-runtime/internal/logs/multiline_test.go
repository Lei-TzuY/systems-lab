package logs

import (
	"testing"
)

func TestAggregateMultilineLogs(t *testing.T) {
	logContent := "Exception in thread main\n\tat com.example.Main.main(Main.java:10)\nServer started\n"
	events := AggregateMultilineLogs(logContent)
	if len(events) != 2 {
		t.Fatalf("AggregateMultilineLogs len = %d, want 2 events", len(events))
	}
}
