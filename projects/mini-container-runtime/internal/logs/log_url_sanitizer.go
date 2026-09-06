// Package logs provides container log processing utilities.
// This file implements a URL sanitizer that strips basic authentication credentials
// and masks sensitive query parameters from URLs found in container logs.

package logs

import (
	"fmt"
	"net/url"
	"regexp"
	"strings"
)

// URLSanitizer detects URLs in log lines and masks credentials & sensitive query params.
type URLSanitizer struct {
	urlRegex            *regexp.Regexp
	SensitiveParamNames map[string]bool
}

// NewURLSanitizer creates a URLSanitizer with standard sensitive query parameter targets.
func NewURLSanitizer(customSensitiveParams []string) *URLSanitizer {
	sensitive := map[string]bool{
		"token":         true,
		"access_token":  true,
		"api_key":       true,
		"apikey":        true,
		"secret":        true,
		"client_secret": true,
		"password":      true,
		"passwd":        true,
		"auth":          true,
		"key":           true,
	}
	for _, p := range customSensitiveParams {
		sensitive[strings.ToLower(p)] = true
	}

	return &URLSanitizer{
		urlRegex:            regexp.MustCompile(`https?://[^\s"'<>]+`),
		SensitiveParamNames: sensitive,
	}
}

// SanitizeURL sanitizes a single URL string.
func (s *URLSanitizer) SanitizeURL(rawURL string) string {
	u, err := url.Parse(rawURL)
	if err != nil {
		return rawURL
	}

	// Mask userinfo password
	if u.User != nil {
		if _, hasPass := u.User.Password(); hasPass {
			u.User = url.UserPassword(u.User.Username(), "REDACTED")
		}
	}

	// Mask sensitive query parameters
	if u.RawQuery != "" {
		values := u.Query()
		modified := false
		for k := range values {
			if s.SensitiveParamNames[strings.ToLower(k)] {
				values.Set(k, "[REDACTED]")
				modified = true
			}
		}
		if modified {
			u.RawQuery = values.Encode()
		}
	}

	return u.String()
}

// SanitizeLine finds all URLs in a line and replaces them with sanitized versions.
func (s *URLSanitizer) SanitizeLine(line string) string {
	return s.urlRegex.ReplaceAllStringFunc(line, func(match string) string {
		return s.SanitizeURL(match)
	})
}

// SanitizeLines processes a slice of log lines, returning sanitized copies.
func (s *URLSanitizer) SanitizeLines(lines []string) []string {
	out := make([]string, len(lines))
	for i, line := range lines {
		out[i] = s.SanitizeLine(line)
	}
	return out
}

// FormatURLSanitizationStats returns a summary count of sanitized URLs.
func FormatURLSanitizationStats(totalLines int, sanitizedCount int) string {
	return fmt.Sprintf("Processed %d lines, sanitized %d URLs", totalLines, sanitizedCount)
}
