// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an extractor for the config.Domainname field from
// an OCI Image Config JSON, returning the declared network domain name.

package imagestore

import (
	"encoding/json"
	"fmt"
)

// domainConfig is the subset of OCI Image Config used for domainname extraction.
type domainConfig struct {
	Config struct {
		Domainname string `json:"Domainname"`
	} `json:"config"`
}

// ExtractDomainname parses an OCI Image Config JSON blob and returns
// the container's declared network domain name (config.Domainname).
// Returns an empty string if the field is not set.
func ExtractDomainname(configJSON []byte) (string, error) {
	var cfg domainConfig
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return "", fmt.Errorf("parse image config for domainname: %w", err)
	}
	return cfg.Config.Domainname, nil
}

// FormatDomainname returns a human-readable summary of the image domain name.
func FormatDomainname(configJSON []byte) string {
	dn, err := ExtractDomainname(configJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}
	if dn == "" {
		return "(not set)"
	}
	return fmt.Sprintf("Domainname: %s", dn)
}
