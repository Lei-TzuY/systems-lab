//go:build !linux

package state

import "os"

type pinnedStateStorage struct {
	rootDir string
	ctrDir  string
	imgDir  string
	files   []*os.File
}

func pinStateStorage(root string) (*pinnedStateStorage, error) {
	return &pinnedStateStorage{
		rootDir: root,
		ctrDir:  root + string(os.PathSeparator) + "containers",
		imgDir:  root + string(os.PathSeparator) + "images",
	}, nil
}

func closePinnedStateStorage(*pinnedStateStorage) {}
