package network

import (
	"sort"
)

// OrderDNSSearchDomains sorts search domains by length and preference.
func OrderDNSSearchDomains(domains []string) []string {
	if len(domains) == 0 {
		return nil
	}
	ordered := make([]string, len(domains))
	copy(ordered, domains)
	sort.SliceStable(ordered, func(i, j int) bool {
		return len(ordered[i]) > len(ordered[j])
	})
	return ordered
}
