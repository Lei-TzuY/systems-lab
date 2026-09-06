package logs

import (
	"strings"
	"testing"
)

func TestLineNumberAnnotator_AnnotateLines(t *testing.T) {
	annotator := NewLineNumberAnnotator(1, ": ", 4)
	lines := []string{"hello", "world", "test"}
	got := annotator.AnnotateLines(lines)

	expected := []string{
		"   1: hello",
		"   2: world",
		"   3: test",
	}

	for i, want := range expected {
		if got[i] != want {
			t.Errorf("line %d: got %q, want %q", i, got[i], want)
		}
	}
}

func TestLineNumberAnnotator_CustomStart(t *testing.T) {
	annotator := NewLineNumberAnnotator(100, " | ", 6)
	got := annotator.Annotate(100, "data")
	if !strings.Contains(got, "100") || !strings.Contains(got, " | ") {
		t.Errorf("unexpected format: %q", got)
	}
}

func TestLineNumberAnnotator_Defaults(t *testing.T) {
	annotator := NewLineNumberAnnotator(-1, "", 0)
	if annotator.StartNum != 1 {
		t.Errorf("expected default start 1, got %d", annotator.StartNum)
	}
	if annotator.Separator != ": " {
		t.Errorf("expected default separator, got %q", annotator.Separator)
	}
	if annotator.PadWidth != 4 {
		t.Errorf("expected default pad 4, got %d", annotator.PadWidth)
	}
}

func TestFormatLineCount(t *testing.T) {
	lines := []string{"hello", "", "  ", "world"}
	got := FormatLineCount(lines)
	if !strings.Contains(got, "4 lines") {
		t.Errorf("expected 4 lines, got %q", got)
	}
	if !strings.Contains(got, "2 non-empty") {
		t.Errorf("expected 2 non-empty, got %q", got)
	}
}
