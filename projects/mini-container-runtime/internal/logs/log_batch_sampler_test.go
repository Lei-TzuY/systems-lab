package logs

import (
	"fmt"
	"testing"
)

func TestSampleEveryN(t *testing.T) {
	lines := []string{"0", "1", "2", "3", "4", "5", "6", "7", "8", "9"}

	sampled := SampleEveryN(lines, 3)
	if len(sampled) != 4 {
		t.Fatalf("len(sampled) = %d, want 4 (0, 3, 6, 9)", len(sampled))
	}
	if sampled[0] != "0" || sampled[1] != "3" || sampled[2] != "6" || sampled[3] != "9" {
		t.Errorf("unexpected sampled lines: %v", sampled)
	}

	all := SampleEveryN(lines, 1)
	if len(all) != len(lines) {
		t.Errorf("len(all) = %d, want %d", len(all), len(lines))
	}
}

func TestSampleFraction(t *testing.T) {
	lines := make([]string, 1000)
	for i := 0; i < 1000; i++ {
		lines[i] = fmt.Sprintf("line-%d", i)
	}

	sampled := SampleFraction(lines, 0.1, 42)
	// Expect roughly 10% (80 to 120 lines)
	if len(sampled) < 70 || len(sampled) > 130 {
		t.Errorf("len(sampled) = %d, expected ~100", len(sampled))
	}

	zero := SampleFraction(lines, 0.0, 42)
	if len(zero) != 0 {
		t.Errorf("expected 0 for rate 0.0, got %d", len(zero))
	}

	full := SampleFraction(lines, 1.0, 42)
	if len(full) != 1000 {
		t.Errorf("expected 1000 for rate 1.0, got %d", len(full))
	}
}

func TestReservoirSample(t *testing.T) {
	lines := make([]string, 500)
	for i := 0; i < 500; i++ {
		lines[i] = fmt.Sprintf("log-%d", i)
	}

	sampled := ReservoirSample(lines, 50, 12345)
	if len(sampled) != 50 {
		t.Fatalf("len(sampled) = %d, want 50", len(sampled))
	}

	short := []string{"a", "b"}
	sampledShort := ReservoirSample(short, 10, 1)
	if len(sampledShort) != 2 {
		t.Errorf("len(sampledShort) = %d, want 2", len(sampledShort))
	}
}
