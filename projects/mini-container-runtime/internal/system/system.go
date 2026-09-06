package system

import (
	"errors"
	"fmt"
	"strings"

	"minicontainer/internal/imagestore"
	"minicontainer/internal/state"
	"minicontainer/internal/volume"
)

type DFResult struct {
	ContainersCount int   `json:"containers_count"`
	ContainersSize  int64 `json:"containers_size"`
	ImagesCount     int   `json:"images_count"`
	ImagesSize      int64 `json:"images_size"`
	VolumesCount    int   `json:"volumes_count"`
	VolumesSize     int64 `json:"volumes_size"`
}

type PruneResult struct {
	ContainersReclaimed int   `json:"containers_reclaimed"`
	ImagesReclaimed     int   `json:"images_reclaimed"`
	VolumesReclaimed    int   `json:"volumes_reclaimed"`
	SpaceReclaimed      int64 `json:"space_reclaimed"`
}

// CalculateDF computes engine storage usage across containers, images, and volumes.
func CalculateDF(st *state.Store) (*DFResult, error) {
	if st == nil {
		return nil, fmt.Errorf("state store is nil")
	}

	ctrs, err := st.List()
	if err != nil {
		return nil, err
	}

	imgs, err := st.ListImages()
	if err != nil {
		return nil, err
	}

	vols, err := volume.ListVolumes()
	if err != nil {
		return nil, err
	}

	res := &DFResult{
		ContainersCount: len(ctrs),
		ImagesCount:     len(imgs),
		VolumesCount:    len(vols),
	}

	for _, c := range ctrs {
		if c == nil {
			return res, fmt.Errorf("container list contains nil metadata")
		}
		if c.RootFS == "" {
			continue
		}
		sz, err := imagestore.CalculateDirSize(c.RootFS)
		if err != nil {
			return res, fmt.Errorf("calculate rootfs size for container %s at %q: %w", c.ID, c.RootFS, err)
		}
		res.ContainersSize += sz
	}

	for _, img := range imgs {
		if img == nil {
			return res, fmt.Errorf("image list contains nil metadata")
		}
		sz := img.Size
		if sz == 0 && img.RootFS != "" {
			sz, err = imagestore.CalculateDirSize(img.RootFS)
			if err != nil {
				selector := strings.TrimSpace(img.Name)
				if selector == "" {
					selector = strings.TrimSpace(img.ID)
				}
				return res, fmt.Errorf("calculate rootfs size for image %q at %q: %w", selector, img.RootFS, err)
			}
		}
		res.ImagesSize += sz
	}

	for _, v := range vols {
		res.VolumesSize += v.Size
	}

	return res, nil
}

// deletePrunableContainer rechecks lifecycle state under the Store lock. A
// concurrent restart is a benign skip; every other refusal/failure must remain
// visible to callers so garbage collection cannot report false success.
func deletePrunableContainer(st *state.Store, id string) (bool, error) {
	err := st.DeleteIfNotRunning(id)
	if err == nil {
		return true, nil
	}
	if errors.Is(err, state.ErrContainerRunning) {
		return false, nil
	}
	return false, err
}

// SystemPrune cleans up stopped containers, unused images, and unused volumes.
func SystemPrune(st *state.Store, pruneAll bool) (*PruneResult, error) {
	if st == nil {
		return nil, fmt.Errorf("state store is nil")
	}
	res := &PruneResult{}

	// Prune stopped containers. DeleteIfNotRunning rechecks disk under the state
	// lock, so a generation restarted after List cannot be removed here. Only
	// that classified lifecycle race is a skip; cleanup/state failures surface.
	ctrs, err := st.List()
	if err != nil {
		return res, fmt.Errorf("list containers for system prune: %w", err)
	}
	for _, c := range ctrs {
		if c.Status != state.StatusStopped {
			continue
		}
		deleted, err := deletePrunableContainer(st, c.ID)
		if err != nil {
			return res, fmt.Errorf("prune stopped container %s: %w", c.ID, err)
		}
		if deleted {
			res.ContainersReclaimed++
		}
	}

	// Volume prune already aggregates per-volume removal failures. Preserve its
	// successful partial count, but never report the overall system prune as
	// successful when any managed volume could not be validated or removed.
	volsCount, err := volume.PruneVolumes()
	res.VolumesReclaimed = volsCount
	if err != nil {
		return res, fmt.Errorf("prune volumes during system prune: %w", err)
	}

	if !pruneAll {
		return res, nil
	}

	imgs, err := st.ListImages()
	if err != nil {
		return res, fmt.Errorf("list images for system prune: %w", err)
	}
	for _, img := range imgs {
		if img == nil {
			return res, fmt.Errorf("image list contains nil metadata")
		}
		selector := strings.TrimSpace(img.Name)
		if selector == "" {
			selector = strings.TrimSpace(img.ID)
		}
		if selector == "" {
			return res, fmt.Errorf("cannot prune unnamed image without an ID")
		}
		if _, err := imagestore.RemoveImageIfMatch(st, selector, img, true); err != nil {
			return res, fmt.Errorf("prune image %q: %w", selector, err)
		}
		res.ImagesReclaimed++
	}

	return res, nil
}
