// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an environment variable expansion engine for OCI image configs
// resolving nested variable interpolations ($VAR, ${VAR}, ${VAR:-default}, ${VAR:+alt}) in sequential order.

package imagestore

import (
	"encoding/json"
	"fmt"
	"regexp"
	"strings"
)

var (
	// envVarBracedRegex matches ${VAR}, ${VAR:-def}, ${VAR-def}, ${VAR:+alt}, ${VAR+alt}
	envVarBracedRegex = regexp.MustCompile(`\$\{([a-zA-Z_][a-zA-Z0-9_]*)(?::?([-+]))?([^}]*)\}`)
	envVarSimpleRegex = regexp.MustCompile(`\$([a-zA-Z_][a-zA-Z0-9_]*)`)
)

const escapeSentinel = "\x00ESCAPED_DOLLAR\x00"

// ExpandEnvString expands variable references in a value string using an environment map.
// Evaluation is strictly isolated to envMap and does not leak host process environment.
func ExpandEnvString(val string, envMap map[string]string) string {
	if !strings.Contains(val, "$") {
		return val
	}

	// 1. Handle $$ escape sequence (literal $)
	res := strings.ReplaceAll(val, "$$", escapeSentinel)

	// 2. Expand braced variables: ${VAR}, ${VAR:-def}, ${VAR-def}, ${VAR:+alt}, ${VAR+alt}
	res = envVarBracedRegex.ReplaceAllStringFunc(res, func(m string) string {
		sub := envVarBracedRegex.FindStringSubmatch(m)
		if len(sub) < 2 {
			return m
		}
		varName := sub[1]
		op := sub[2]       // "-" or "+" (with or without colon prefix captured via regex)
		arg := sub[3]      // default or alternate value
		colon := strings.HasPrefix(m[len(varName)+2:], ":")

		val, exists := envMap[varName]

		switch op {
		case "-":
			if colon {
				// ${VAR:-def}: use def if unset or empty
				if !exists || val == "" {
					return arg
				}
				return val
			}
			// ${VAR-def}: use def if unset
			if !exists {
				return arg
			}
			return val
		case "+":
			if colon {
				// ${VAR:+alt}: use alt if set and non-empty
				if exists && val != "" {
					return arg
				}
				return ""
			}
			// ${VAR+alt}: use alt if set
			if exists {
				return arg
			}
			return ""
		default:
			// Plain ${VAR}
			if exists {
				return val
			}
			return ""
		}
	})

	// 3. Expand simple $VAR
	res = envVarSimpleRegex.ReplaceAllStringFunc(res, func(m string) string {
		sub := envVarSimpleRegex.FindStringSubmatch(m)
		if len(sub) < 2 {
			return m
		}
		varName := sub[1]
		if v, exists := envMap[varName]; exists {
			return v
		}
		return ""
	})

	// 4. Restore escaped dollars
	res = strings.ReplaceAll(res, escapeSentinel, "$")

	return res
}

// ResolveImageEnvironment parses an Image Config JSON blob and returns the fully resolved environment slice.
func ResolveImageEnvironment(configJSON []byte) ([]string, error) {
	var cfg struct {
		Config struct {
			Env []string `json:"Env,omitempty"`
		} `json:"config"`
	}
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return nil, fmt.Errorf("parse image config for env resolution: %w", err)
	}

	envMap := make(map[string]string)
	var resolved []string

	for _, entry := range cfg.Config.Env {
		parts := strings.SplitN(entry, "=", 2)
		if len(parts) == 0 || parts[0] == "" {
			continue
		}
		key := parts[0]
		rawVal := ""
		if len(parts) == 2 {
			rawVal = parts[1]
		}

		expandedVal := ExpandEnvString(rawVal, envMap)
		envMap[key] = expandedVal
		resolved = append(resolved, fmt.Sprintf("%s=%s", key, expandedVal))
	}

	return resolved, nil
}

// FormatResolvedEnvironment returns a formatted summary of resolved environment variables.
func FormatResolvedEnvironment(configJSON []byte) string {
	envs, err := ResolveImageEnvironment(configJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}
	if len(envs) == 0 {
		return "Environment: (none declared)"
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("Environment: %d variables (fully resolved)\n", len(envs)))
	for i, e := range envs {
		sb.WriteString(fmt.Sprintf("  [%d] %s\n", i+1, e))
	}
	return strings.TrimRight(sb.String(), "\n")
}
