package logs

import (
	"strings"
	"testing"
)

func TestCalculateEntropy(t *testing.T) {
	t.Run("empty string has zero entropy", func(t *testing.T) {
		if got := CalculateEntropy(""); got != 0.0 {
			t.Errorf("got %f, want 0.0", got)
		}
	})

	t.Run("single repeated character has zero entropy", func(t *testing.T) {
		if got := CalculateEntropy("AAAAAAAAAA"); got != 0.0 {
			t.Errorf("got %f, want 0.0", got)
		}
	})

	t.Run("high entropy string", func(t *testing.T) {
		highEntropy := "4kL9#mP!8xQz2$wV7@jR5%tY"
		ent := CalculateEntropy(highEntropy)
		if ent < 4.0 {
			t.Errorf("expected high entropy (>4.0), got %f", ent)
		}
	})
}

func TestEntropyFilter_FilterLines(t *testing.T) {
	lines := []string{
		"aaaaaaaaaaaaaaaaaaaaaaaa",       // low entropy (~0)
		"hello world hello world",        // low-medium entropy (~2.8)
		"aB9$zX8#mK2!qW5%vR7@jP4&xT1*cY", // very high entropy (>4.5)
	}

	filter := NewEntropyFilter(4.2, 8.0)
	got := filter.FilterLines(lines)

	if len(got) != 1 {
		t.Fatalf("expected 1 high-entropy line, got %d: %v", len(got), got)
	}
	if got[0] != "aB9$zX8#mK2!qW5%vR7@jP4&xT1*cY" {
		t.Errorf("unexpected filtered line: %q", got[0])
	}
}

func TestFormatEntropyStats(t *testing.T) {
	got := FormatEntropyStats("test message")
	if !strings.Contains(got, "[entropy:") {
		t.Errorf("expected [entropy: in %q", got)
	}
}
