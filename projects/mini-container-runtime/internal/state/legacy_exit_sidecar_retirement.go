package state

import "fmt"

// retireLegacyStoppedGenerationSidecarsUnlocked removes upgrade-only stopped
// generation sidecars after lifecycle JSON has durably become authoritative.
// Removal happens only after a versioned embedded identity is committed; a
// crash between the JSON commit and this cleanup is safe because versioned
// readers never consult these files and will retry retirement on the next read.
func (s *Store) retireLegacyStoppedGenerationSidecarsUnlocked(id string) error {
	if err := validateID(id); err != nil {
		return err
	}
	if err := removeStateFileDurable(s.ctrDir, exitedIdentityPath(s.ctrDir, id), "exited process identity"); err != nil {
		return fmt.Errorf("retire legacy exited process identity: %w", err)
	}
	if err := removeStateFileDurable(s.ctrDir, exitedIdentityRequiredPath(s.ctrDir, id), "exited identity requirement"); err != nil {
		return fmt.Errorf("retire legacy exited identity requirement: %w", err)
	}
	return nil
}
