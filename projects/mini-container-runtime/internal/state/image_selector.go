package state

import (
	"fmt"
	"sort"
)

type imageSelectorMatches struct {
	named   []*Image
	exactID []*Image
	byID    map[string][]*Image
}

func collectImageSelectorMatches(images []*Image, selector string) imageSelectorMatches {
	matches := imageSelectorMatches{byID: make(map[string][]*Image)}
	for _, img := range images {
		if img == nil {
			continue
		}
		if img.Name == selector || (img.Repository+":"+img.Tag) == selector {
			matches.named = append(matches.named, img)
		}
		if img.ID == selector {
			matches.exactID = append(matches.exactID, img)
			continue
		}
		if img.ID != "" && len(img.ID) > len(selector) && img.ID[:len(selector)] == selector {
			matches.byID[img.ID] = append(matches.byID[img.ID], img)
		}
	}
	return matches
}

func sortImageAliases(images []*Image) {
	sort.Slice(images, func(i, j int) bool {
		if images[i].Name != images[j].Name {
			return images[i].Name < images[j].Name
		}
		if images[i].Repository != images[j].Repository {
			return images[i].Repository < images[j].Repository
		}
		return images[i].Tag < images[j].Tag
	})
}

func validateAliasRootFS(aliases []*Image) error {
	if len(aliases) < 2 {
		return nil
	}
	id := aliases[0].ID
	rootFS := aliases[0].RootFS
	for _, img := range aliases[1:] {
		if img.ID != id {
			return fmt.Errorf("inconsistent image aliases: expected ID %s, got %s", id, img.ID)
		}
		if img.RootFS != rootFS {
			return fmt.Errorf("inconsistent aliases for image ID %s reference different rootfs paths", id)
		}
	}
	return nil
}

func resolveAliasSetForRead(aliases []*Image) (*Image, error) {
	if len(aliases) == 0 {
		return nil, fmt.Errorf("image not found")
	}
	if err := validateAliasRootFS(aliases); err != nil {
		return nil, err
	}
	sortImageAliases(aliases)
	return aliases[0], nil
}

func resolveAliasSetForDelete(id string, aliases []*Image) (*Image, error) {
	if len(aliases) == 0 {
		return nil, fmt.Errorf("image %q not found", id)
	}
	if len(aliases) > 1 {
		names := make([]string, 0, len(aliases))
		for _, img := range aliases {
			name := img.Name
			if name == "" {
				name = "<unnamed>"
			}
			names = append(names, name)
		}
		sort.Strings(names)
		return nil, fmt.Errorf("image ID %s has multiple tags (%v); specify an image name or tag", id, names)
	}
	return aliases[0], nil
}

func hasNamedExactIDCollision(matches imageSelectorMatches) bool {
	if len(matches.named) == 0 || len(matches.exactID) == 0 {
		return false
	}
	for _, named := range matches.named {
		for _, exact := range matches.exactID {
			if named != exact {
				return true
			}
	}
	}
	return false
}

func resolveImageForRead(images []*Image, selector string) (*Image, error) {
	matches := collectImageSelectorMatches(images, selector)
	if len(matches.named) > 1 {
		return nil, fmt.Errorf("ambiguous image selector %q matched multiple named images", selector)
	}
	if hasNamedExactIDCollision(matches) {
		return nil, fmt.Errorf("ambiguous image selector %q matched both an image name/tag and an exact image ID", selector)
	}
	if len(matches.named) == 1 {
		return matches.named[0], nil
	}
	if len(matches.exactID) > 0 {
		return resolveAliasSetForRead(matches.exactID)
	}
	if len(matches.byID) == 0 {
		return nil, fmt.Errorf("image %q not found", selector)
	}
	if len(matches.byID) > 1 {
		ids := make([]string, 0, len(matches.byID))
		for id := range matches.byID {
			ids = append(ids, id)
		}
		sort.Strings(ids)
		return nil, fmt.Errorf("ambiguous image ID prefix %q matched multiple IDs (%v)", selector, ids)
	}
	for _, aliases := range matches.byID {
		return resolveAliasSetForRead(aliases)
	}
	return nil, fmt.Errorf("image %q not found", selector)
}

func resolveImageForDelete(images []*Image, selector string) (*Image, error) {
	matches := collectImageSelectorMatches(images, selector)
	if len(matches.named) > 1 {
		return nil, fmt.Errorf("ambiguous image selector %q matched multiple named images", selector)
	}
	if hasNamedExactIDCollision(matches) {
		return nil, fmt.Errorf("ambiguous image selector %q matched both an image name/tag and an exact image ID", selector)
	}
	if len(matches.named) == 1 {
		return matches.named[0], nil
	}
	if len(matches.exactID) > 0 {
		return resolveAliasSetForDelete(selector, matches.exactID)
	}
	if len(matches.byID) == 0 {
		return nil, fmt.Errorf("image %q not found", selector)
	}
	if len(matches.byID) > 1 {
		ids := make([]string, 0, len(matches.byID))
		for id := range matches.byID {
			ids = append(ids, id)
		}
		sort.Strings(ids)
		return nil, fmt.Errorf("ambiguous image ID prefix %q matched multiple IDs (%v)", selector, ids)
	}
	for id, aliases := range matches.byID {
		return resolveAliasSetForDelete(id, aliases)
	}
	return nil, fmt.Errorf("image %q not found", selector)
}
