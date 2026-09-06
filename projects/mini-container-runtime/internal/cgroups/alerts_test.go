package cgroups

import (
	"testing"
)

func TestCheckMemoryAlert(t *testing.T) {
	if !CheckMemoryAlert(150, 100) {
		t.Fatalf("CheckMemoryAlert should return true when usage exceeds limit")
	}
	if CheckMemoryAlert(50, 100) {
		t.Fatalf("CheckMemoryAlert should return false when usage is below limit")
	}
}
