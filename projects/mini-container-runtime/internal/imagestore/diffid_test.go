package imagestore

import (
	"testing"
)

func TestVerifyDiffIDs(t *testing.T) {
	diffs1 := []string{"sha256:1111", "sha256:2222"}
	diffs2 := []string{"sha256:1111", "sha256:2222"}

	valid, err := VerifyDiffIDs(diffs1, diffs2)
	if err != nil || !valid {
		t.Fatalf("VerifyDiffIDs error: %v (valid=%v)", err, valid)
	}
}
