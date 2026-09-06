// internal/image/image.go
//
// Container Image Unpacking
// ──────────────────────────
// A container image is, at its core, a layered tar archive. OCI images
// consist of one or more layers stacked into a root filesystem.

package image

import (
	"archive/tar"
	"compress/gzip"
	"fmt"
	"io"
	"os"
	"path"
	"path/filepath"
	"strings"
)

// Unpack extracts a tar or tar.gz archive to destDir, creating destDir if
// needed. It prints a summary line on completion.
func Unpack(tarPath, destDir string) error {
	f, err := os.Open(tarPath)
	if err != nil {
		return fmt.Errorf("open %s: %w", tarPath, err)
	}
	defer f.Close()

	var reader io.Reader = f
	lower := strings.ToLower(tarPath)
	if strings.HasSuffix(lower, ".gz") || strings.HasSuffix(lower, ".tgz") {
		gz, err := gzip.NewReader(f)
		if err != nil {
			return fmt.Errorf("gzip: %w", err)
		}
		defer gz.Close()
		reader = gz
	}

	destDir = filepath.Clean(destDir)
	if err := os.MkdirAll(destDir, 0755); err != nil {
		return fmt.Errorf("mkdir %s: %w", destDir, err)
	}

	tr := tar.NewReader(reader)
	var extracted int
	var directoryMetadataToFinalize []directoryMetadata
	var pendingHardlinks deferredHardlinks
	for {
		hdr, err := tr.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return fmt.Errorf("reading tar entry: %w", err)
		}
		target, err := safePath(destDir, hdr.Name)
		if err != nil {
			return err
		}
		if err := applyTarEntryWithDeferredHardlinks(target, hdr, tr, destDir, &pendingHardlinks); err != nil {
			return err
		}
		if hdr.Typeflag == tar.TypeDir {
			directoryMetadataToFinalize = append(directoryMetadataToFinalize, directoryMetadata{
				target:  target,
				mode:    hdr.FileInfo().Mode(),
				modTime: hdr.ModTime,
				uid:     hdr.Uid,
				gid:     hdr.Gid,
				xattrs:  tarXattrsPortable(hdr),
			})
		}
		extracted++
	}

	if err := pendingHardlinks.finish(destDir); err != nil {
		return err
	}
	if err := finalizeDirectoryMetadata(destDir, directoryMetadataToFinalize); err != nil {
		return fmt.Errorf("finalize directory metadata: %w", err)
	}
	fmt.Printf("Extracted %d entries → %s\n", extracted, destDir)
	return nil
}

// applyTarEntry writes a single tar entry to the filesystem under destDir.
// Linux entry implementations traverse from pinned dirfds; non-Linux keeps the
// legacy pathname parent preparation through prepareTarEntryParent.
func applyTarEntry(target string, hdr *tar.Header, r io.Reader, destDir string) error {
	if err := prepareTarEntryParent(target, destDir); err != nil {
		return err
	}

	switch hdr.Typeflag {
	case tar.TypeDir:
		return createDirectorySecure(target, destDir, hdr.FileInfo().Mode()|0700)
	case tar.TypeReg, tar.TypeRegA:
		return writeRegularSecure(target, destDir, hdr, r)
	case tar.TypeSymlink:
		return createTarSymlinkSecure(target, destDir, hdr)
	case tar.TypeLink:
		linkTarget, err := safePath(destDir, hdr.Linkname)
		if err != nil {
			return err
		}
		return createTarHardlinkSecure(target, destDir, linkTarget, hdr)
	case tar.TypeChar, tar.TypeBlock, tar.TypeFifo:
		if err := makeSpecialSecure(target, destDir, hdr); err != nil {
			return fmt.Errorf("create special tar entry %q (type %d): %w", hdr.Name, hdr.Typeflag, err)
		}
		return nil
	default:
		return fmt.Errorf("unsupported tar entry %q with type flag %d", hdr.Name, hdr.Typeflag)
	}
}

func ensureSafeParentDirs(target, destDir string) error {
	destAbs, err := filepath.Abs(destDir)
	if err != nil { return err }
	targetAbs, err := filepath.Abs(target)
	if err != nil { return err }
	rel, err := filepath.Rel(destAbs, targetAbs)
	if err != nil || rel == "." || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return fmt.Errorf("path traversal detected: %q escapes %q", target, destDir)
	}
	parts := strings.Split(filepath.Dir(rel), string(filepath.Separator))
	curr := destAbs
	for _, part := range parts {
		if part == "." || part == "" { continue }
		curr = filepath.Join(curr, part)
		fi, err := os.Lstat(curr)
		if err == nil {
			if fi.Mode()&os.ModeSymlink != 0 {
				eval, err := filepath.EvalSymlinks(curr)
				if err != nil || !isSubDir(destAbs, eval) {
					return fmt.Errorf("symlink path traversal detected: directory component %q escapes destination", curr)
				}
			}
		} else if os.IsNotExist(err) {
			if err := os.Mkdir(curr, 0755); err != nil && !os.IsExist(err) { return err }
		} else { return err }
	}
	return nil
}

func isSubDir(base, target string) bool {
	baseAbs, err1 := filepath.Abs(base)
	targetAbs, err2 := filepath.Abs(target)
	if err1 != nil || err2 != nil { return false }
	rel, err := filepath.Rel(baseAbs, targetAbs)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) { return false }
	return true
}

func safePath(base, name string) (string, error) {
	baseAbs, err := filepath.Abs(base)
	if err != nil { return "", fmt.Errorf("resolve destination %q: %w", base, err) }
	normalized := strings.ReplaceAll(name, "\\", "/")
	if strings.HasPrefix(normalized, "/") { return "", fmt.Errorf("path traversal detected: %q escapes destination", name) }
	trimmed := strings.TrimLeft(normalized, "/")
	if hasWindowsDrivePrefix(trimmed) { return "", fmt.Errorf("path traversal detected: %q escapes destination", name) }
	for _, part := range strings.Split(normalized, "/") {
		if part == ".." { return "", fmt.Errorf("path traversal detected: %q escapes destination", name) }
	}
	cleaned := strings.TrimPrefix(path.Clean("/"+normalized), "/")
	if cleaned == "." { cleaned = "" }
	target := filepath.Join(baseAbs, filepath.FromSlash(cleaned))
	rel, err := filepath.Rel(baseAbs, target)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(os.PathSeparator)) {
		return "", fmt.Errorf("path traversal detected: %q escapes destination", name)
	}
	return target, nil
}

func hasWindowsDrivePrefix(name string) bool {
	return len(name) >= 2 && name[1] == ':' && ((name[0] >= 'a' && name[0] <= 'z') || (name[0] >= 'A' && name[0] <= 'Z'))
}
