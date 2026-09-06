//go:build !linux

package image

import (
	"archive/tar"
	"fmt"
	"io"
	"os"
)

func writeRegularSecure(target, destDir string, hdr *tar.Header, r io.Reader) error {
	if _, err := os.Lstat(target); err == nil {
		if err := os.RemoveAll(target); err != nil && !os.IsNotExist(err) { return fmt.Errorf("remove existing node before write %s: %w", target, err) }
	} else if !os.IsNotExist(err) { return fmt.Errorf("inspect existing node before write %s: %w", target, err) }
	return writeRegular(target, hdr, r)
}

func writeRegular(target string, hdr *tar.Header, r io.Reader) error {
	out, err := os.OpenFile(target, os.O_CREATE|os.O_EXCL|os.O_WRONLY, hdr.FileInfo().Mode())
	if err != nil { return fmt.Errorf("create %s exclusively: %w", target, err) }
	if _, err := io.Copy(out, r); err != nil { _ = out.Close(); return fmt.Errorf("write %s: %w", target, err) }
	return out.Close()
}
