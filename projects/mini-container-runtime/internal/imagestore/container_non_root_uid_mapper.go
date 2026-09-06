// Package imagestore provides OCI image configuration inspection utilities.
// This file implements user mapping validation for rootless user namespaces (userns),
// evaluating numeric and symbolic UID/GID definitions in Image Configs.

package imagestore

import (
	"encoding/json"
	"fmt"
	"strconv"
	"strings"
)

// MaxValidPOSIXID is the maximum valid POSIX UID/GID (4294967294), reserving 4294967295 as (uid_t)-1.
const MaxValidPOSIXID = 4294967294

// UserNamespaceConfig contains parsed user execution details and userns mapping status.
type UserNamespaceConfig struct {
	RawUser           string
	UID               int64
	GID               int64
	IsRoot            bool
	IsNumeric         bool
	IsRootlessAllowed bool
}

// SubIDRange represents a range in /etc/subuid or /etc/subgid.
type SubIDRange struct {
	StartID uint32
	Length  uint32
}

// ValidateUserNamespaceMapping evaluates if the container image user can be mapped
// into a rootless user namespace range.
func ValidateUserNamespaceMapping(configJSON []byte, hostSubUIDRange SubIDRange) (UserNamespaceConfig, error) {
	var cfg struct {
		Config struct {
			User string `json:"User,omitempty"`
		} `json:"config"`
	}
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return UserNamespaceConfig{}, fmt.Errorf("parse image config for user mapping: %w", err)
	}

	raw := strings.TrimSpace(cfg.Config.User)
	res := UserNamespaceConfig{
		RawUser: raw,
		UID:     0,
		GID:     0,
		IsRoot:  false,
	}

	if hostSubUIDRange.Length == 0 {
		res.IsRootlessAllowed = false
	}

	// Validate colon count (at most one colon for user:group)
	if strings.Count(raw, ":") > 1 {
		return UserNamespaceConfig{}, fmt.Errorf("invalid user specification %q: too many colons", raw)
	}

	parts := strings.Split(raw, ":")
	userPart := strings.TrimSpace(parts[0])
	groupPart := ""
	if len(parts) == 2 {
		groupPart = strings.TrimSpace(parts[1])
	}

	if userPart == "" || userPart == "root" || userPart == "0" {
		res.IsRoot = true
		res.IsNumeric = (userPart == "0" || userPart == "")
		res.UID = 0

		if groupPart != "" {
			gidParsed, errG := strconv.ParseInt(groupPart, 10, 64)
			if errG == nil {
				if gidParsed < 0 || gidParsed > MaxValidPOSIXID {
					return UserNamespaceConfig{}, fmt.Errorf("invalid GID %d: out of valid range (0-%d)", gidParsed, MaxValidPOSIXID)
				}
				res.GID = gidParsed
			} else if groupPart == "root" || groupPart == "0" {
				res.GID = 0
			} else {
				res.GID = 65534
			}
		} else {
			res.GID = 0
		}
	} else {
		uidParsed, errU := strconv.ParseInt(userPart, 10, 64)
		if errU == nil {
			if uidParsed < 0 || uidParsed > MaxValidPOSIXID {
				return UserNamespaceConfig{}, fmt.Errorf("invalid UID %d: out of valid range (0-%d)", uidParsed, MaxValidPOSIXID)
			}
			res.UID = uidParsed
			res.IsNumeric = true
			if res.UID == 0 {
				res.IsRoot = true
			}

			if groupPart != "" {
				gidParsed, errG := strconv.ParseInt(groupPart, 10, 64)
				if errG == nil {
					if gidParsed < 0 || gidParsed > MaxValidPOSIXID {
						return UserNamespaceConfig{}, fmt.Errorf("invalid GID %d: out of valid range (0-%d)", gidParsed, MaxValidPOSIXID)
					}
					res.GID = gidParsed
				} else if groupPart == "root" || groupPart == "0" {
					res.GID = 0
				} else {
					res.GID = 65534
				}
			}
		} else {
			// Non-numeric user (e.g. "nobody", "www-data", "appuser")
			res.IsNumeric = false
			res.UID = 65534 // fallback nobody (0xFFFE)
			res.GID = 65534

			if groupPart != "" {
				gidParsed, errG := strconv.ParseInt(groupPart, 10, 64)
				if errG == nil {
					if gidParsed < 0 || gidParsed > MaxValidPOSIXID {
						return UserNamespaceConfig{}, fmt.Errorf("invalid GID %d: out of valid range (0-%d)", gidParsed, MaxValidPOSIXID)
					}
					res.GID = gidParsed
				} else if groupPart == "root" || groupPart == "0" {
					res.GID = 0
				}
			}
		}
	}

	if hostSubUIDRange.Length > 0 {
		if res.IsRoot {
			res.IsRootlessAllowed = true
		} else if uint64(res.UID) < uint64(hostSubUIDRange.Length) {
			res.IsRootlessAllowed = true
		}
	}

	return res, nil
}

// FormatUserNamespaceMapping returns a human-readable summary of user namespace compatibility.
func FormatUserNamespaceMapping(configJSON []byte, hostRange SubIDRange) string {
	mapping, err := ValidateUserNamespaceMapping(configJSON, hostRange)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("User Namespace Mapping Evaluation:\n"))
	sb.WriteString(fmt.Sprintf("  Config User: %q\n", mapping.RawUser))
	sb.WriteString(fmt.Sprintf("  Parsed UID:GID: %d:%d (numeric: %t)\n", mapping.UID, mapping.GID, mapping.IsNumeric))
	sb.WriteString(fmt.Sprintf("  Runs as Root: %t\n", mapping.IsRoot))
	sb.WriteString(fmt.Sprintf("  Rootless Mode Compatible: %t", mapping.IsRootlessAllowed))
	return sb.String()
}
