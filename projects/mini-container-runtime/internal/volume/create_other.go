//go:build !linux

package volume

import "time"

func createVolume(name string, createdAt time.Time) (*Volume, error) {
	root, err := volumeRoot(true)
	if err != nil {
		return nil, err
	}
	volDir, dataPath, err := ensureVolumeLayout(root, name, true)
	if err != nil {
		return nil, err
	}
	vol := &Volume{Name: name, MountPath: dataPath, CreatedAt: createdAt}
	if err := writeVolumeMetadata(volDir, vol); err != nil {
		return nil, err
	}
	return vol, nil
}
