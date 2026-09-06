package imagestore

import (
	"fmt"
)

// VerifyDiffIDs checks if calculated layer diffIDs match image config descriptors.
func VerifyDiffIDs(configDiffIDs, calculatedDiffIDs []string) (bool, error) {
	if len(configDiffIDs) != len(calculatedDiffIDs) {
		return false, fmt.Errorf("diffID count mismatch: config has %d, calculated has %d", len(configDiffIDs), len(calculatedDiffIDs))
	}

	for i := range configDiffIDs {
		if configDiffIDs[i] != calculatedDiffIDs[i] {
			return false, fmt.Errorf("diffID mismatch at layer %d: config=%s, calculated=%s", i, configDiffIDs[i], calculatedDiffIDs[i])
		}
	}

	return true, nil
}
