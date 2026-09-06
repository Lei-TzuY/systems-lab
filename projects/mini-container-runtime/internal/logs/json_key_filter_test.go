package logs

import (
	"encoding/json"
	"reflect"
	"testing"
)

func TestJSONKeyFilter_FilterLine(t *testing.T) {
	tests := []struct {
		name        string
		keys        []string
		dropNonJSON bool
		input       string
		wantMap     map[string]interface{}
		wantRaw     string
	}{
		{
			name:        "keeps requested keys only",
			keys:        []string{"level", "msg"},
			dropNonJSON: false,
			input:       `{"level":"info","msg":"started","pid":1234,"trace_id":"xyz"}`,
			wantMap: map[string]interface{}{
				"level": "info",
				"msg":   "started",
			},
		},
		{
			name:        "non-json line kept when dropNonJSON=false",
			keys:        []string{"level"},
			dropNonJSON: false,
			input:       "plain text line",
			wantRaw:     "plain text line",
		},
		{
			name:        "non-json line dropped when dropNonJSON=true",
			keys:        []string{"level"},
			dropNonJSON: true,
			input:       "plain text line",
			wantRaw:     "",
		},
		{
			name:        "no matching keys returns empty",
			keys:        []string{"nonexistent"},
			dropNonJSON: false,
			input:       `{"level":"info","msg":"ok"}`,
			wantRaw:     "",
		},
		{
			name:        "empty filter keys returns original",
			keys:        []string{},
			dropNonJSON: false,
			input:       `{"level":"info"}`,
			wantRaw:     `{"level":"info"}`,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			filter := NewJSONKeyFilter(tc.keys, tc.dropNonJSON)
			got := filter.FilterLine(tc.input)

			if tc.wantMap != nil {
				var gotMap map[string]interface{}
				if err := json.Unmarshal([]byte(got), &gotMap); err != nil {
					t.Fatalf("failed to unmarshal output %q: %v", got, err)
				}
				if !reflect.DeepEqual(gotMap, tc.wantMap) {
					t.Errorf("got map %v, want %v", gotMap, tc.wantMap)
				}
			} else {
				if got != tc.wantRaw {
					t.Errorf("FilterLine() = %q, want %q", got, tc.wantRaw)
				}
			}
		})
	}
}

func TestJSONKeyFilter_FilterLines(t *testing.T) {
	filter := NewJSONKeyFilter([]string{"status"}, true)
	lines := []string{
		`{"host":"web1","status":200,"latency":15}`,
		"plain log line to be dropped",
		`{"host":"web2","status":500}`,
		`{"other":"value"}`,
	}

	got := filter.FilterLines(lines)
	if len(got) != 2 {
		t.Fatalf("FilterLines() returned %d lines, want 2", len(got))
	}

	var m1, m2 map[string]interface{}
	_ = json.Unmarshal([]byte(got[0]), &m1)
	_ = json.Unmarshal([]byte(got[1]), &m2)

	if m1["status"] != float64(200) || m2["status"] != float64(500) {
		t.Errorf("unexpected filtered results: %v", got)
	}
}
