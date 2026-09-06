package imagestore

import (
	"encoding/json"
	"sort"
)

type ConfigWithVolumes struct {
	Config struct {
		Volumes map[string]struct{} `json:"Volumes"`
	} `json:"config"`
}

// ExtractVolumes extracts list of declared volume paths from Image Config JSON.
func ExtractVolumes(configJSON []byte) []string {
	var cfg ConfigWithVolumes
	if err := json.Unmarshal(configJSON, &cfg); err == nil {
		if cfg.Config.Volumes != nil {
			var result []string
			for vol := range cfg.Config.Volumes {
				result = append(result, vol)
			}
			sort.Strings(result)
			return result
		}
	}
	return nil
}
