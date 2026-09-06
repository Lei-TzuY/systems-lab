package logs

import (
	"strings"
	"testing"
)

func TestHeadLines(t *testing.T) {
	lines := []string{"a", "b", "c", "d", "e"}

	got := HeadLines(lines, 3)
	if len(got) != 3 || got[0] != "a" || got[2] != "c" {
		t.Errorf("HeadLines(5, 3) = %v, want [a b c]", got)
	}

	got = HeadLines(lines, 0)
	if len(got) != 5 {
		t.Errorf("HeadLines(5, 0) should return all, got %d", len(got))
	}

	got = HeadLines(lines, 10)
	if len(got) != 5 {
		t.Errorf("HeadLines(5, 10) should return all, got %d", len(got))
	}
}

func TestTailLines(t *testing.T) {
	lines := []string{"a", "b", "c", "d", "e"}

	got := TailLines(lines, 2)
	if len(got) != 2 || got[0] != "d" || got[1] != "e" {
		t.Errorf("TailLines(5, 2) = %v, want [d e]", got)
	}

	got = TailLines(lines, 0)
	if len(got) != 5 {
		t.Errorf("TailLines(5, 0) should return all, got %d", len(got))
	}
}

func TestHeadTailLines(t *testing.T) {
	lines := []string{"1", "2", "3", "4", "5", "6", "7", "8", "9", "10"}

	got := HeadTailLines(lines, 3, 2)
	if len(got) != 6 {
		t.Fatalf("expected 6 lines (3 head + 1 separator + 2 tail), got %d: %v", len(got), got)
	}
	if got[0] != "1" || got[2] != "3" {
		t.Errorf("head portion wrong: %v", got[:3])
	}
	if !strings.Contains(got[3], "skipped 5 lines") {
		t.Errorf("separator wrong: %q", got[3])
	}
	if got[4] != "9" || got[5] != "10" {
		t.Errorf("tail portion wrong: %v", got[4:])
	}
}

func TestHeadTailLines_ShortStream(t *testing.T) {
	lines := []string{"a", "b"}
	got := HeadTailLines(lines, 5, 5)
	if len(got) != 2 {
		t.Errorf("short stream should return all lines, got %d", len(got))
	}
}
