// Package logs provides container log processing utilities.
// This file implements a regex-based secret and sensitive data redactor
// to mask credentials, API keys, tokens, and PII in container log streams.

package logs

import (
	"fmt"
	"regexp"
)

// RedactionRule defines a regex pattern and its replacement mask.
type RedactionRule struct {
	Name        string
	Pattern     *regexp.Regexp
	Replacement string
}

// LogRedactor applies redaction rules to sanitize log records.
type LogRedactor struct {
	rules []RedactionRule
}

// NewDefaultLogRedactor creates a LogRedactor with standard secret and credential patterns.
func NewDefaultLogRedactor() *LogRedactor {
	redactor := &LogRedactor{}

	// Standard Bearer tokens
	redactor.AddRule("Bearer Token", `(?i)bearer\s+[A-Za-z0-9\-\._~\+\/]+=*`, "Bearer [REDACTED_TOKEN]")

	// Password and secret query params or JSON keys
	redactor.AddRule("Password Field", `(?i)(password|passwd|secret|apikey|api_key|token)["']?\s*[:=]\s*["']?([^"',\s]+)["']?`, `$1=[REDACTED]`)

	// JWT tokens (three base64 segments joined by dots)
	redactor.AddRule("JWT Token", `eyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}`, "[REDACTED_JWT]")

	// Email addresses
	redactor.AddRule("Email Address", `[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}`, "[REDACTED_EMAIL]")

	return redactor
}

// AddRule compiles and appends a custom regex redaction rule.
func (r *LogRedactor) AddRule(name, pattern, replacement string) error {
	re, err := regexp.Compile(pattern)
	if err != nil {
		return fmt.Errorf("compile rule %q regex %q: %w", name, pattern, err)
	}
	r.rules = append(r.rules, RedactionRule{
		Name:        name,
		Pattern:     re,
		Replacement: replacement,
	})
	return nil
}

// RedactLine applies all configured redaction rules to a single log line.
func (r *LogRedactor) RedactLine(line string) string {
	result := line
	for _, rule := range r.rules {
		result = rule.Pattern.ReplaceAllString(result, rule.Replacement)
	}
	return result
}

// RedactLines processes a slice of log lines, returning sanitized copies.
func (r *LogRedactor) RedactLines(lines []string) []string {
	out := make([]string, len(lines))
	for i, line := range lines {
		out[i] = r.RedactLine(line)
	}
	return out
}
