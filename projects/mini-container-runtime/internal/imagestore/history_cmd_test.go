package imagestore

import (
	"testing"
)

func TestCleanHistoryCommand(t *testing.T) {
	raw := "/bin/sh -c #(nop)  CMD [\"/bin/sh\"]"
	cleaned := CleanHistoryCommand(raw)
	if cleaned != "CMD [\"/bin/sh\"]" {
		t.Fatalf("CleanHistoryCommand = %s, want CMD [\"/bin/sh\"]", cleaned)
	}
}
