// Package logs provides container log processing utilities.
// This file implements an IP address anonymizer that masks IPv4 and IPv6
// addresses in log streams for GDPR/compliance privacy controls.

package logs

import (
	"fmt"
	"regexp"
)

// IPMaskMode defines how IP addresses are masked.
type IPMaskMode int

const (
	// MaskFull replaces the entire IP address with a placeholder e.g. [IP_MASKED].
	MaskFull IPMaskMode = iota
	// MaskLastOctet masks only the last octet e.g. 192.168.1.xxx.
	MaskLastOctet
)

// IPMasker detects and sanitizes IP addresses in container logs.
type IPMasker struct {
	Mode      IPMaskMode
	ipv4Regex *regexp.Regexp
	ipv6Regex *regexp.Regexp
}

// NewIPMasker creates a new IPMasker.
func NewIPMasker(mode IPMaskMode) *IPMasker {
	return &IPMasker{
		Mode:      mode,
		ipv4Regex: regexp.MustCompile(`\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b`),
		ipv6Regex: regexp.MustCompile(`\b(?:[0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}\b|\b(?:[0-9a-fA-F]{1,4}:){1,7}:|::(?:[0-9a-fA-F]{1,4}:){0,6}[0-9a-fA-F]{1,4}\b`),
	}
}

// MaskLine detects and masks all IP addresses in a single line.
func (m *IPMasker) MaskLine(line string) string {
	// Process IPv4
	res := m.ipv4Regex.ReplaceAllStringFunc(line, func(ip string) string {
		if m.Mode == MaskFull {
			return "[IPv4_MASKED]"
		}
		// Replace last octet
		lastDot := -1
		for i := len(ip) - 1; i >= 0; i-- {
			if ip[i] == '.' {
				lastDot = i
				break
			}
		}
		if lastDot != -1 {
			return fmt.Sprintf("%s.xxx", ip[:lastDot])
		}
		return "[IPv4_MASKED]"
	})

	// Process IPv6
	res = m.ipv6Regex.ReplaceAllString(res, "[IPv6_MASKED]")

	return res
}

// MaskLines processes a slice of log lines, returning sanitized copies.
func (m *IPMasker) MaskLines(lines []string) []string {
	out := make([]string, len(lines))
	for i, line := range lines {
		out[i] = m.MaskLine(line)
	}
	return out
}
