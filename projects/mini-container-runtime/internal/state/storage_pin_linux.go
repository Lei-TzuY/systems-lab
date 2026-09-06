//go:build linux

package state

import (
	"fmt"
	"os"
	"strconv"

	"golang.org/x/sys/unix"
)

type pinnedStateStorage struct {
	rootDir string
	ctrDir  string
	imgDir  string
	files   []*os.File
}

func pinStateStorage(root string) (*pinnedStateStorage, error) {
	rootFile, err := openPinnedStateDir(root, -1, "", "state")
	if err != nil {
		return nil, err
	}
	pins := []*os.File{rootFile}
	closePins := func() {
		for i := len(pins) - 1; i >= 0; i-- {
			_ = pins[i].Close()
		}
	}

	ctrFile, err := openPinnedStateDir("", int(rootFile.Fd()), "containers", "container state")
	if err != nil {
		closePins()
		return nil, err
	}
	pins = append(pins, ctrFile)

	imgFile, err := openPinnedStateDir("", int(rootFile.Fd()), "images", "image state")
	if err != nil {
		closePins()
		return nil, err
	}
	pins = append(pins, imgFile)

	return &pinnedStateStorage{
		rootDir: procFDPath(rootFile),
		ctrDir:  procFDPath(ctrFile),
		imgDir:  procFDPath(imgFile),
		files:   pins,
	}, nil
}

func openPinnedStateDir(path string, parentFD int, name, label string) (*os.File, error) {
	flags := unix.O_RDONLY | unix.O_DIRECTORY | unix.O_CLOEXEC | unix.O_NOFOLLOW
	var (
		fd  int
		err error
	)
	if parentFD >= 0 {
		fd, err = unix.Openat(parentFD, name, flags, 0)
	} else {
		fd, err = unix.Open(path, flags, 0)
	}
	if err != nil {
		return nil, fmt.Errorf("pin %s directory: %w", label, err)
	}
	file := os.NewFile(uintptr(fd), path)
	if file == nil {
		_ = unix.Close(fd)
		return nil, fmt.Errorf("wrap pinned %s directory fd", label)
	}

	var st unix.Stat_t
	if err := unix.Fstat(fd, &st); err != nil {
		_ = file.Close()
		return nil, fmt.Errorf("inspect pinned %s directory: %w", label, err)
	}
	if st.Mode&unix.S_IFMT != unix.S_IFDIR {
		_ = file.Close()
		return nil, fmt.Errorf("pinned %s path is not a directory", label)
	}
	if err := unix.Fchmod(fd, 0o700); err != nil {
		_ = file.Close()
		return nil, fmt.Errorf("secure pinned %s directory: %w", label, err)
	}
	return file, nil
}

func procFDPath(file *os.File) string {
	return "/proc/self/fd/" + strconv.FormatUint(uint64(file.Fd()), 10)
}

func closePinnedStateStorage(pinned *pinnedStateStorage) {
	if pinned == nil {
		return
	}
	for i := len(pinned.files) - 1; i >= 0; i-- {
		_ = pinned.files[i].Close()
	}
}
