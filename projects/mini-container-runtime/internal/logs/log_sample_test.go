package logs

import (
	"reflect"
	"testing"
)

func TestLogSampler_Interval(t *testing.T) {
	lines := []string{"L1", "L2", "L3", "L4", "L5", "L6", "L7", "L8", "L9", "L10"}

	t.Run("every 3rd line", func(t *testing.T) {
		s := NewIntervalSampler(3)
		got := s.SampleLines(lines)
		want := []string{"L1", "L4", "L7", "L10"}
		if !reflect.DeepEqual(got, want) {
			t.Errorf("got %v, want %v", got, want)
		}
	})

	t.Run("every 1st line (all lines)", func(t *testing.T) {
		s := NewIntervalSampler(1)
		got := s.SampleLines(lines)
		if !reflect.DeepEqual(got, lines) {
			t.Errorf("got %v, want %v", got, lines)
		}
	})

	t.Run("invalid interval defaults to 1", func(t *testing.T) {
		s := NewIntervalSampler(-5)
		if s.Interval != 1 {
			t.Errorf("Interval = %d, want 1", s.Interval)
		}
	})
}

func TestLogSampler_Rate(t *testing.T) {
	t.Run("rate 0.0 samples nothing", func(t *testing.T) {
		s := NewRateSampler(0.0, 42)
		lines := []string{"L1", "L2", "L3"}
		got := s.SampleLines(lines)
		if len(got) != 0 {
			t.Errorf("expected 0 lines, got %v", got)
		}
	})

	t.Run("rate 1.0 samples everything", func(t *testing.T) {
		s := NewRateSampler(1.0, 42)
		lines := []string{"L1", "L2", "L3"}
		got := s.SampleLines(lines)
		if !reflect.DeepEqual(got, lines) {
			t.Errorf("got %v, want %v", got, lines)
		}
	})
}

func TestFormatSamplingStats(t *testing.T) {
	t.Run("non-zero", func(t *testing.T) {
		got := FormatSamplingStats(100, 25)
		want := "25/100 lines sampled (25.0%)"
		if got != want {
			t.Errorf("got %q, want %q", got, want)
		}
	})

	t.Run("zero", func(t *testing.T) {
		got := FormatSamplingStats(0, 0)
		want := "0 lines sampled (0%)"
		if got != want {
			t.Errorf("got %q, want %q", got, want)
		}
	})
}
