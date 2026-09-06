// Package logs provides container log processing utilities.
// This file implements a JSON log field projection filter that parses structured
// JSON log lines and outputs projected JSON objects containing only requested keys.

package logs

import (
	"encoding/json"
	"fmt"
)

// JSONKeyFilter holds the list of JSON keys to retain in projected log lines.
type JSONKeyFilter struct {
	// Keys is the list of top-level JSON keys to keep.
	Keys []string
	// DropNonJSON specifies whether non-JSON log lines should be dropped (omitted).
	DropNonJSON bool
}

// NewJSONKeyFilter creates a new JSON key projection filter.
func NewJSONKeyFilter(keys []string, dropNonJSON bool) *JSONKeyFilter {
	return &JSONKeyFilter{
		Keys:        keys,
		DropNonJSON: dropNonJSON,
	}
}

// FilterLine parses a single log line. If it is valid JSON, it returns a new JSON
// string containing only the specified keys. If it is not JSON, it either returns
// the original line or an empty string depending on DropNonJSON.
func (jf *JSONKeyFilter) FilterLine(line string) string {
	if len(jf.Keys) == 0 {
		return line
	}

	var raw map[string]interface{}
	if err := json.Unmarshal([]byte(line), &raw); err != nil {
		if jf.DropNonJSON {
			return ""
		}
		return line
	}

	wanted := make(map[string]bool, len(jf.Keys))
	for _, k := range jf.Keys {
		wanted[k] = true
	}

	projected := make(map[string]interface{})
	for k, v := range raw {
		if wanted[k] {
			projected[k] = v
		}
	}

	if len(projected) == 0 {
		return ""
	}

	out, err := json.Marshal(projected)
	if err != nil {
		return fmt.Sprintf("json marshal error: %v", err)
	}
	return string(out)
}

// FilterLines processes a slice of log lines and returns the filtered results.
func (jf *JSONKeyFilter) FilterLines(lines []string) []string {
	var result []string
	for _, line := range lines {
		out := jf.FilterLine(line)
		if out != "" {
			result = append(result, out)
		}
	}
	return result
}
