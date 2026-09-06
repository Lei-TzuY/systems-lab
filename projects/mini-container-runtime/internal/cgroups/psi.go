package cgroups

import (
	"bufio"
	"fmt"
	"math"
	"strconv"
	"strings"
)

// PSIValues contains one Pressure Stall Information (PSI) sample line.
// Avg10, Avg60, and Avg300 are percentages; Total is cumulative stall time
// in microseconds.
type PSIValues struct {
	Avg10  float64 `json:"avg10"`
	Avg60  float64 `json:"avg60"`
	Avg300 float64 `json:"avg300"`
	Total  uint64  `json:"total"`
}

// PSIStats preserves both PSI scopes exposed by the kernel. Full is optional
// because older kernels and some CPU PSI interfaces may omit it.
type PSIStats struct {
	Some PSIValues  `json:"some"`
	Full *PSIValues `json:"full,omitempty"`
}

func validatePSIResource(resource string) error {
	switch resource {
	case "cpu", "memory", "io":
		return nil
	default:
		return fmt.Errorf("unsupported PSI resource %q", resource)
	}
}

func parsePSI(data []byte) (*PSIStats, error) {
	stats := &PSIStats{}
	seenSome := false
	seenFull := false

	scanner := bufio.NewScanner(strings.NewReader(string(data)))
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" {
			continue
		}

		fields := strings.Fields(line)
		if len(fields) < 2 {
			return nil, fmt.Errorf("malformed PSI line %q", line)
		}

		scope := fields[0]
		if scope != "some" && scope != "full" {
			// Ignore unknown future scopes while remaining strict about the
			// fields in the scopes we understand.
			continue
		}

		values, err := parsePSIValues(scope, fields[1:])
		if err != nil {
			return nil, err
		}

		switch scope {
		case "some":
			if seenSome {
				return nil, fmt.Errorf("duplicate PSI some line")
			}
			stats.Some = values
			seenSome = true
		case "full":
			if seenFull {
				return nil, fmt.Errorf("duplicate PSI full line")
			}
			full := values
			stats.Full = &full
			seenFull = true
		}
	}
	if err := scanner.Err(); err != nil {
		return nil, fmt.Errorf("scan PSI data: %w", err)
	}
	if !seenSome {
		return nil, fmt.Errorf("PSI data missing some line")
	}

	return stats, nil
}

func parsePSIValues(scope string, fields []string) (PSIValues, error) {
	var values PSIValues
	seen := make(map[string]bool, 4)

	for _, field := range fields {
		key, raw, ok := strings.Cut(field, "=")
		if !ok || key == "" || raw == "" {
			return PSIValues{}, fmt.Errorf("malformed PSI %s field %q", scope, field)
		}

		switch key {
		case "avg10", "avg60", "avg300", "total":
			if seen[key] {
				return PSIValues{}, fmt.Errorf("duplicate PSI %s field %q", scope, key)
			}
			seen[key] = true
		default:
			// Kernel interfaces can grow additional keyed fields. Ignore keys
			// we do not consume so this parser remains forward compatible.
			continue
		}

		switch key {
		case "avg10", "avg60", "avg300":
			value, err := strconv.ParseFloat(raw, 64)
			if err != nil {
				return PSIValues{}, fmt.Errorf("parse PSI %s %s: %w", scope, key, err)
			}
			if math.IsNaN(value) || math.IsInf(value, 0) || value < 0 || value > 100 {
				return PSIValues{}, fmt.Errorf("PSI %s %s out of range: %q", scope, key, raw)
			}
			switch key {
			case "avg10":
				values.Avg10 = value
			case "avg60":
				values.Avg60 = value
			case "avg300":
				values.Avg300 = value
			}
		case "total":
			value, err := strconv.ParseUint(raw, 10, 64)
			if err != nil {
				return PSIValues{}, fmt.Errorf("parse PSI %s total: %w", scope, err)
			}
			values.Total = value
		}
	}

	for _, required := range []string{"avg10", "avg60", "avg300", "total"} {
		if !seen[required] {
			return PSIValues{}, fmt.Errorf("PSI %s line missing %s", scope, required)
		}
	}

	return values, nil
}
