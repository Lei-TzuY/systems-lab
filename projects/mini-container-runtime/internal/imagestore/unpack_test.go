package imagestore

import (
	"testing"
)

func TestVerifyRootFSTree(t *testing.T) {
	tmpDir := t.TempDir()
	valid, err := VerifyRootFSTree(tmpDir)
	if err != nil || !valid {
		t.Fatalf("VerifyRootFSTree error: %v (valid=%v)", err, valid)
	}
}
