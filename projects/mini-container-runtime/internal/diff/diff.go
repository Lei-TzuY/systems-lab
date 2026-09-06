package diff

import (
	"bytes"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

type ChangeType string

const (
	Added   ChangeType = "A"
	Changed ChangeType = "C"
	Deleted ChangeType = "D"
)

type Change struct {
	Type ChangeType `json:"type"`
	Path string     `json:"path"`
}

// DiffUpper inspects an OverlayFS upper directory and categorizes changes.
func DiffUpper(upperDir string) ([]Change, error) {
	var changes []Change

	err := filepath.WalkDir(upperDir, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if path == upperDir {
			return nil
		}

		rel, err := filepath.Rel(upperDir, path)
		if err != nil {
			return fmt.Errorf("diff rel path %s: %w", path, err)
		}

		relPath := "/" + filepath.ToSlash(rel)
		baseName := d.Name()

		// OverlayFS whiteout character file (deleted file marker `.wh.<filename>`)
		if strings.HasPrefix(baseName, ".wh.") {
			deletedName := strings.TrimPrefix(baseName, ".wh.")
			delRel := filepath.Join(filepath.Dir(rel), deletedName)
			changes = append(changes, Change{
				Type: Deleted,
				Path: "/" + filepath.ToSlash(delRel),
			})
			return nil
		}

		changes = append(changes, Change{
			Type: Added,
			Path: relPath,
		})
		return nil
	})

	if err != nil {
		return nil, fmt.Errorf("diff upper dir: %w", err)
	}

	sortChanges(changes)
	return changes, nil
}

type fileMeta struct {
	isDir bool
	mode  fs.FileMode
	size  int64
	full  string
}

// DiffDirectories compares targetDir against baseDir file by file with content awareness.
func DiffDirectories(baseDir, targetDir string) ([]Change, error) {
	targetFiles := make(map[string]fileMeta)

	err := filepath.WalkDir(targetDir, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if path == targetDir {
			return nil
		}
		rel, err := filepath.Rel(targetDir, path)
		if err != nil {
			return fmt.Errorf("diff rel target path %s: %w", path, err)
		}
		relPath := "/" + filepath.ToSlash(rel)
		info, err := d.Info()
		if err != nil {
			return fmt.Errorf("inspect file info %s: %w", path, err)
		}
		targetFiles[relPath] = fileMeta{
			isDir: d.IsDir(),
			mode:  info.Mode(),
			size:  info.Size(),
			full:  path,
		}
		return nil
	})
	if err != nil {
		return nil, fmt.Errorf("walk target dir: %w", err)
	}

	baseFiles := make(map[string]fileMeta)
	err = filepath.WalkDir(baseDir, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if path == baseDir {
			return nil
		}
		rel, err := filepath.Rel(baseDir, path)
		if err != nil {
			return fmt.Errorf("diff rel base path %s: %w", path, err)
		}
		relPath := "/" + filepath.ToSlash(rel)
		info, err := d.Info()
		if err != nil {
			return fmt.Errorf("inspect file info %s: %w", path, err)
		}
		baseFiles[relPath] = fileMeta{
			isDir: d.IsDir(),
			mode:  info.Mode(),
			size:  info.Size(),
			full:  path,
		}
		return nil
	})
	if err != nil {
		return nil, fmt.Errorf("walk base dir: %w", err)
	}

	var changes []Change

	for relPath, tMeta := range targetFiles {
		bMeta, exists := baseFiles[relPath]
		if !exists {
			changes = append(changes, Change{Type: Added, Path: relPath})
		} else if !filesEqual(bMeta, tMeta) {
			changes = append(changes, Change{Type: Changed, Path: relPath})
		}
	}

	for relPath := range baseFiles {
		if _, exists := targetFiles[relPath]; !exists {
			changes = append(changes, Change{Type: Deleted, Path: relPath})
		}
	}

	sortChanges(changes)
	return changes, nil
}

func filesEqual(bMeta, tMeta fileMeta) bool {
	if bMeta.isDir && tMeta.isDir {
		return true
	}
	if bMeta.isDir != tMeta.isDir {
		return false
	}
	if bMeta.size != tMeta.size {
		return false
	}
	if bMeta.mode != tMeta.mode {
		return false
	}
	baseData, err1 := os.ReadFile(bMeta.full)
	targetData, err2 := os.ReadFile(tMeta.full)
	if err1 != nil || err2 != nil {
		return false
	}
	return bytes.Equal(baseData, targetData)
}

func sortChanges(changes []Change) {
	sort.Slice(changes, func(i, j int) bool {
		if changes[i].Path == changes[j].Path {
			return changes[i].Type < changes[j].Type
		}
		return changes[i].Path < changes[j].Path
	})
}

func FormatDiff(changes []Change) string {
	var sb strings.Builder
	for _, c := range changes {
		sb.WriteString(fmt.Sprintf("%s %s\n", c.Type, c.Path))
	}
	return sb.String()
}
