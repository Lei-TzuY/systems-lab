package logs

import (
	"reflect"
	"testing"
)

func TestDeduplicateLines(t *testing.T) {
	tests := []struct {
		name  string
		input []string
		want  []DeduplicateResult
	}{
		{
			name:  "empty input",
			input: nil,
			want:  nil,
		},
		{
			name:  "no duplicates",
			input: []string{"a", "b", "c"},
			want: []DeduplicateResult{
				{Line: "a", Count: 1},
				{Line: "b", Count: 1},
				{Line: "c", Count: 1},
			},
		},
		{
			name:  "consecutive duplicates",
			input: []string{"a", "a", "a", "b", "b", "c"},
			want: []DeduplicateResult{
				{Line: "a", Count: 3},
				{Line: "b", Count: 2},
				{Line: "c", Count: 1},
			},
		},
		{
			name:  "non-consecutive same values not grouped",
			input: []string{"a", "b", "a"},
			want: []DeduplicateResult{
				{Line: "a", Count: 1},
				{Line: "b", Count: 1},
				{Line: "a", Count: 1},
			},
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := DeduplicateLines(tc.input)
			if !reflect.DeepEqual(got, tc.want) {
				t.Errorf("DeduplicateLines() = %+v, want %+v", got, tc.want)
			}
		})
	}
}

func TestFormatDeduplicated(t *testing.T) {
	results := []DeduplicateResult{
		{Line: "error: timeout", Count: 5},
		{Line: "info: ok", Count: 1},
	}

	got := FormatDeduplicated(results)
	want := []string{
		"error: timeout [repeated 5 times]",
		"info: ok",
	}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("FormatDeduplicated() = %v, want %v", got, want)
	}
}
