package image

import (
	"archive/tar"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

type deferredHardlink struct {
	target     string
	linkTarget string
	hdr        tar.Header
}

type deferredHardlinks struct {
	entries []deferredHardlink
}

func cloneTarHeader(hdr *tar.Header) tar.Header {
	clone := *hdr
	if hdr.PAXRecords != nil {
		clone.PAXRecords = make(map[string]string, len(hdr.PAXRecords))
		for k, v := range hdr.PAXRecords {
			clone.PAXRecords[k] = v
		}
	}
	if hdr.Xattrs != nil {
		clone.Xattrs = make(map[string]string, len(hdr.Xattrs))
		for k, v := range hdr.Xattrs {
			clone.Xattrs[k] = v
		}
	}
	return clone
}

func (d *deferredHardlinks) cancelTarget(target string) {
	kept := d.entries[:0]
	for _, entry := range d.entries {
		if entry.target != target {
			kept = append(kept, entry)
		}
	}
	d.entries = kept
}

func (d *deferredHardlinks) cancelSubtree(root string, includeRoot bool) {
	kept := d.entries[:0]
	for _, entry := range d.entries {
		rel, err := filepath.Rel(root, entry.target)
		within := err == nil && rel != ".." && !filepath.IsAbs(rel) && !strings.HasPrefix(rel, ".."+string(filepath.Separator))
		if !includeRoot && (rel == "." || rel == "") {
			within = false
		}
		if !within {
			kept = append(kept, entry)
		}
	}
	d.entries = kept
}

// cancelForEntry preserves archive ordering for deferred hardlinks. A later
// directory entry can coexist with older descendants, but any later
// non-directory entry at an ancestor makes those older descendant paths
// unreachable and must invalidate them before they can be resolved later.
func (d *deferredHardlinks) cancelForEntry(target string, typeflag byte) {
	if typeflag == tar.TypeDir {
		d.cancelTarget(target)
		return
	}
	d.cancelSubtree(target, true)
}

func (d *deferredHardlinks) add(target, linkTarget string, hdr *tar.Header) {
	d.entries = append(d.entries, deferredHardlink{
		target:     target,
		linkTarget: linkTarget,
		hdr:        cloneTarHeader(hdr),
	})
}

func (d *deferredHardlinks) resolveAvailable(destDir string) error {
	for {
		progress := false
		kept := make([]deferredHardlink, 0, len(d.entries))
		for _, entry := range d.entries {
			err := createTarHardlinkSecure(entry.target, destDir, entry.linkTarget, &entry.hdr)
			if err == nil {
				progress = true
				continue
			}
			if errors.Is(err, os.ErrNotExist) {
				kept = append(kept, entry)
				continue
			}
			return fmt.Errorf("resolve deferred hardlink %s -> %s: %w", entry.target, entry.linkTarget, err)
		}
		d.entries = kept
		if !progress {
			return nil
		}
	}
}

func (d *deferredHardlinks) finish(destDir string) error {
	if err := d.resolveAvailable(destDir); err != nil {
		return err
	}
	if len(d.entries) == 0 {
		return nil
	}
	entry := d.entries[0]
	return fmt.Errorf("unresolved hardlink %s -> %s: source never appeared in archive", entry.target, entry.linkTarget)
}

func applyTarEntryWithDeferredHardlinks(target string, hdr *tar.Header, r io.Reader, destDir string, pending *deferredHardlinks) error {
	pending.cancelForEntry(target, hdr.Typeflag)
	if hdr.Typeflag == tar.TypeLink {
		linkTarget, err := safePath(destDir, hdr.Linkname)
		if err != nil {
			return err
		}
		if err := createTarHardlinkSecure(target, destDir, linkTarget, hdr); err != nil {
			if !errors.Is(err, os.ErrNotExist) {
				return err
			}
			pending.add(target, linkTarget, hdr)
			return nil
		}
	} else if err := applyTarEntry(target, hdr, r, destDir); err != nil {
		return err
	}
	return pending.resolveAvailable(destDir)
}
