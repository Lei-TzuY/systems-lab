package logs

import (
	"reflect"
	"testing"
)

func TestLogGrepContextFilter_FilterWithContext(t *testing.T) {
	lines := []string{
		"line 0: boot",
		"line 1: step 1",
		"line 2: ERROR in module A",
		"line 3: recovery ok",
		"line 4: normal ops 1",
		"line 5: normal ops 2",
		"line 6: ERROR in module B",
		"line 7: shutdown",
	}

	tests := []struct {
		name      string
		pattern   string
		before    int
		after     int
		separator string
		want      []string
	}{
		{
			name:      "match with before 1 after 1",
			pattern:   "ERROR",
			before:    1,
			after:     1,
			separator: "--",
			want: []string{
				"line 1: step 1",
				"line 2: ERROR in module A",
				"line 3: recovery ok",
				"--",
				"line 5: normal ops 2",
				"line 6: ERROR in module B",
				"line 7: shutdown",
			},
		},
		{
			name:      "no matches returns nil",
			pattern:   "CRITICAL",
			before:    1,
			after:     1,
			separator: "--",
			want:      nil,
		},
		{
			name:      "overlapping context merged smoothly",
			pattern:   "line [23]",
			before:    1,
			after:     1,
			separator: "--",
			want: []string{
				"line 1: step 1",
				"line 2: ERROR in module A",
				"line 3: recovery ok",
				"line 4: normal ops 1",
			},
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			filter, err := NewLogGrepContextFilter(tc.pattern, tc.before, tc.after, tc.separator)
			if err != nil {
				t.Fatalf("NewLogGrepContextFilter failed: %v", err)
			}
			got := filter.FilterWithContext(lines)
			if !reflect.DeepEqual(got, tc.want) {
				t.Errorf("FilterWithContext() =\n%#v\nwant:\n%#v", got, tc.want)
			}
		})
	}
}

func TestLogGrepContextFilter_Validation(t *testing.T) {
	if _, err := NewLogGrepContextFilter("", 0, 0, ""); err == nil {
		t.Error("expected error on empty pattern")
	}
	if _, err := NewLogGrepContextFilter("[invalid", 0, 0, ""); err == nil {
		t.Error("expected error on invalid regex")
	}
}
