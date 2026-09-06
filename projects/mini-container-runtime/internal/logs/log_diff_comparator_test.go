package logs

import (
	"reflect"
	"strings"
	"testing"
)

func TestCompareLogStreams_Identical(t *testing.T) {
	lines := []string{"start", "init database", "ready"}
	diff := CompareLogStreams(lines, lines)

	if diff.SimilarityRatio != 1.0 {
		t.Errorf("SimilarityRatio = %f, want 1.0", diff.SimilarityRatio)
	}
	if len(diff.OnlyInA) != 0 || len(diff.OnlyInB) != 0 {
		t.Errorf("expected 0 divergent lines, got A:%d B:%d", len(diff.OnlyInA), len(diff.OnlyInB))
	}
}

func TestCompareLogStreams_Divergent(t *testing.T) {
	linesA := []string{"start", "connect db", "ready"}
	linesB := []string{"start", "connect db failed", "retry", "ready"}

	diff := CompareLogStreams(linesA, linesB)

	if diff.CommonLines != 2 { // "start", "ready"
		t.Errorf("CommonLines = %d, want 2", diff.CommonLines)
	}
	if len(diff.OnlyInA) != 1 {
		t.Errorf("OnlyInA = %d, want 1 ('connect db')", len(diff.OnlyInA))
	}
	if len(diff.OnlyInB) != 2 {
		t.Errorf("OnlyInB = %d, want 2", len(diff.OnlyInB))
	}
}

func TestCompareLogStreams_DivergentLinesAreDeterministic(t *testing.T) {
	linesA := []string{"zeta", "common", "alpha", "gamma"}
	linesB := []string{"theta", "beta", "common", "delta"}

	diff := CompareLogStreams(linesA, linesB)

	wantA := []string{"alpha", "gamma", "zeta"}
	wantB := []string{"beta", "delta", "theta"}
	if !reflect.DeepEqual(diff.OnlyInA, wantA) {
		t.Fatalf("OnlyInA = %#v, want %#v", diff.OnlyInA, wantA)
	}
	if !reflect.DeepEqual(diff.OnlyInB, wantB) {
		t.Fatalf("OnlyInB = %#v, want %#v", diff.OnlyInB, wantB)
	}
}

func TestCompareLogStreams_UsesSetSemantics(t *testing.T) {
	linesA := []string{"ready", "ready", "  error  ", ""}
	linesB := []string{"ready", "error", "error"}

	diff := CompareLogStreams(linesA, linesB)

	if diff.CommonLines != 2 {
		t.Errorf("CommonLines = %d, want 2 unique normalized lines", diff.CommonLines)
	}
	if diff.SimilarityRatio != 1.0 {
		t.Errorf("SimilarityRatio = %f, want 1.0 for equal normalized line sets", diff.SimilarityRatio)
	}
}

func TestFormatLogStreamDiff(t *testing.T) {
	diff := LogStreamDiff{
		TotalLinesA:     10,
		TotalLinesB:     12,
		CommonLines:     9,
		OnlyInA:         []string{"err A"},
		OnlyInB:         []string{"err B1", "err B2"},
		SimilarityRatio: 0.75,
	}

	got := FormatLogStreamDiff(diff)
	if !strings.Contains(got, "Similarity: 75.0%") {
		t.Errorf("expected 75.0%% in %q", got)
	}
	if !strings.Contains(got, "Divergent lines: 1 only in A, 2 only in B") {
		t.Errorf("expected divergence summary in %q", got)
	}
}
