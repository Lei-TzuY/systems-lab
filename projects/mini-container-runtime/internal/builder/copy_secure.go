package builder

import (
	"fmt"
	"os"
	"path"
)

func copySymlink(src, dstRoot, dstLogical string) error {
	target, err := os.Readlink(src)
	if err != nil {
		return err
	}
	if err := mkdirRootFSPath(dstRoot, path.Dir(dstLogical), 0o755); err != nil {
		return err
	}
	dst, err := resolveRootFSLeaf(dstRoot, dstLogical)
	if err != nil {
		return err
	}
	if err := os.Remove(dst); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("replace destination symlink %q: %w", dstLogical, err)
	}
	if err := os.Symlink(target, dst); err != nil {
		return fmt.Errorf("copy symlink %q: %w", src, err)
	}
	return nil
}

func destinationIsDirectory(root, logical string) (bool, error) {
	hostPath, err := resolveRootFSPath(root, logical)
	if err != nil {
		return false, err
	}
	info, err := os.Stat(hostPath)
	if err != nil {
		if os.IsNotExist(err) {
			return false, nil
		}
		return false, err
	}
	return info.IsDir(), nil
}
