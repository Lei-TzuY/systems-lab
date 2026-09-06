package dns

import (
	"fmt"
	"net"
	"regexp"
	"strings"
)

var validSearchDomainRegex = regexp.MustCompile(`^[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$`)

// GenerateResolvConf formats custom nameserver IPs and search domain rules with injection defense.
func GenerateResolvConf(nameservers []string, searchDomains []string) string {
	var lines []string

	var validDomains []string
	for _, domain := range searchDomains {
		cleaned := strings.TrimSpace(domain)
		if cleaned != "" && !strings.ContainsAny(cleaned, "\r\n\t ") && validSearchDomainRegex.MatchString(cleaned) {
			validDomains = append(validDomains, cleaned)
		}
	}

	if len(validDomains) > 0 {
		lines = append(lines, fmt.Sprintf("search %s", strings.Join(validDomains, " ")))
	}

	var validNS []string
	for _, ns := range nameservers {
		cleaned := strings.TrimSpace(ns)
		if cleaned != "" && !strings.ContainsAny(cleaned, "\r\n\t ") && net.ParseIP(cleaned) != nil {
			validNS = append(validNS, cleaned)
		}
	}

	for _, ns := range validNS {
		lines = append(lines, fmt.Sprintf("nameserver %s", ns))
	}

	if len(validNS) == 0 {
		lines = append(lines, "nameserver 1.1.1.1", "nameserver 8.8.8.8")
	}

	return strings.Join(lines, "\n") + "\n"
}
