package logs

import (
	"reflect"
	"testing"
)

func TestColumnExtractor_ExtractLine(t *testing.T) {
	tests := []struct {
		name      string
		columns   []int
		delimiter string
		outSep    string
		line      string
		expected  string
	}{
		{
			name:      "whitespace columns 1 and 3",
			columns:   []int{1, 3},
			delimiter: "",
			outSep:    " ",
			line:      "2026-08-20 GET /api/v1/users 200 45ms",
			expected:  "2026-08-20 /api/v1/users",
		},
		{
			name:      "csv columns 2 and 4",
			columns:   []int{2, 4},
			delimiter: ",",
			outSep:    " | ",
			line:      "id123,alice,admin,active,us-east",
			expected:  "alice | active",
		},
		{
			name:      "column index out of bounds skipped",
			columns:   []int{1, 99},
			delimiter: " ",
			outSep:    " ",
			line:      "foo bar baz",
			expected:  "foo",
		},
		{
			name:      "all indices out of bounds returns empty",
			columns:   []int{5, 6},
			delimiter: " ",
			outSep:    " ",
			line:      "foo bar",
			expected:  "",
		},
		{
			name:      "empty columns returns original line",
			columns:   []int{},
			delimiter: " ",
			outSep:    " ",
			line:      "foo bar",
			expected:  "foo bar",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			ce := NewColumnExtractor(tc.columns, tc.delimiter, tc.outSep)
			got := ce.ExtractLine(tc.line)
			if got != tc.expected {
				t.Errorf("ExtractLine() = %q, want %q", got, tc.expected)
			}
		})
	}
}

func TestColumnExtractor_ExtractLines(t *testing.T) {
	ce := NewColumnExtractor([]int{1, 2}, ",", ",")
	lines := []string{
		"a,b,c",
		"single_item_no_second_col",
		"d,e,f",
	}

	got := ce.ExtractLines(lines)
	want := []string{
		"a,b",
		"single_item_no_second_col",
		"d,e",
	}

	if !reflect.DeepEqual(got, want) {
		t.Errorf("ExtractLines() = %v, want %v", got, want)
	}
}
