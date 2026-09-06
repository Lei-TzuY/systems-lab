package logs

import (
	"strings"
	"testing"
)

func TestColorHighlighter_HighlightSeverities(t *testing.T) {
	ch := NewColorHighlighter(true)

	t.Run("highlights error in red", func(t *testing.T) {
		line := "2026-08-20 [ERROR] failed to connect"
		got := ch.HighlightLine(line)
		if !strings.Contains(got, ColorRed) || !strings.Contains(got, ColorReset) {
			t.Errorf("expected Red ANSI code in %q", got)
		}
	})

	t.Run("highlights info in green", func(t *testing.T) {
		line := "2026-08-20 [INFO] server ready"
		got := ch.HighlightLine(line)
		if !strings.Contains(got, ColorGreen) || !strings.Contains(got, ColorReset) {
			t.Errorf("expected Green ANSI code in %q", got)
		}
	})

	t.Run("highlights warn in yellow", func(t *testing.T) {
		line := "2026-08-20 [WARN] cache high memory"
		got := ch.HighlightLine(line)
		if !strings.Contains(got, ColorYellow) || !strings.Contains(got, ColorReset) {
			t.Errorf("expected Yellow ANSI code in %q", got)
		}
	})

	t.Run("highlights debug in cyan", func(t *testing.T) {
		line := "2026-08-20 [DEBUG] query trace"
		got := ch.HighlightLine(line)
		if !strings.Contains(got, ColorCyan) || !strings.Contains(got, ColorReset) {
			t.Errorf("expected Cyan ANSI code in %q", got)
		}
	})
}

func TestColorHighlighter_CustomKeyword(t *testing.T) {
	ch := NewColorHighlighter(false)
	err := ch.AddKeyword("TOKEN_SECRET", ColorMagenta)
	if err != nil {
		t.Fatalf("AddKeyword failed: %v", err)
	}

	line := "authorized with TOKEN_SECRET now"
	got := ch.HighlightLine(line)
	if !strings.Contains(got, ColorMagenta) {
		t.Errorf("expected Magenta in %q", got)
	}

	lines := []string{"foo TOKEN_SECRET", "bar normal"}
	highlighted := ch.HighlightLines(lines)
	if !strings.Contains(highlighted[0], ColorMagenta) {
		t.Errorf("expected Magenta in lines[0]")
	}
	if strings.Contains(highlighted[1], ColorMagenta) {
		t.Errorf("unexpected Magenta in lines[1]")
	}
}

func TestColorHighlighter_InvalidRegex(t *testing.T) {
	ch := NewColorHighlighter(false)
	err := ch.AddKeyword("[invalid(", ColorRed)
	if err == nil {
		t.Fatal("expected error for invalid regex, got nil")
	}
}
