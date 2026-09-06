package registry

import (
	"archive/tar"
	"compress/gzip"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"time"

	"minicontainer/internal/state"
)

type OCIManifest struct {
	SchemaVersion int             `json:"schemaVersion"`
	MediaType     string          `json:"mediaType"`
	Config        OCIDescriptor   `json:"config"`
	Layers        []OCIDescriptor `json:"layers"`
}

type OCIDescriptor struct {
	MediaType string `json:"mediaType"`
	Digest    string `json:"digest"`
	Size      int64  `json:"size"`
}

func atomicPublishFile(path string, data []byte, mode os.FileMode) error {
	dir := filepath.Dir(path)
	tmp, err := os.CreateTemp(dir, "."+filepath.Base(path)+".tmp-*")
	if err != nil {
		return fmt.Errorf("create temporary output: %w", err)
	}
	tmpName := tmp.Name()
	closed := false
	published := false
	defer func() {
		if !closed {
			_ = tmp.Close()
		}
		if !published {
			_ = os.Remove(tmpName)
		}
	}()
	if err := tmp.Chmod(mode); err != nil {
		return fmt.Errorf("set temporary output mode: %w", err)
	}
	if _, err := tmp.Write(data); err != nil {
		return fmt.Errorf("write temporary output: %w", err)
	}
	if err := tmp.Sync(); err != nil {
		return fmt.Errorf("sync temporary output: %w", err)
	}
	if err := tmp.Close(); err != nil {
		return fmt.Errorf("close temporary output: %w", err)
	}
	closed = true
	if err := os.Rename(tmpName, path); err != nil {
		return fmt.Errorf("publish output: %w", err)
	}
	published = true
	return nil
}

// BuildOCILayer packages a rootfs directory into a tar.gz blob and returns SHA256 digest and byte size.
// The destination is only replaced after the complete tar/gzip stream is closed and synced.
func BuildOCILayer(rootfsDir string, destArchive string) (string, int64, error) {
	rootInfo, err := os.Stat(rootfsDir)
	if err != nil {
		return "", 0, fmt.Errorf("stat rootfs: %w", err)
	}
	if !rootInfo.IsDir() {
		return "", 0, fmt.Errorf("rootfs %q is not a directory", rootfsDir)
	}

	destDir := filepath.Dir(destArchive)
	tmp, err := os.CreateTemp(destDir, "."+filepath.Base(destArchive)+".tmp-*")
	if err != nil {
		return "", 0, fmt.Errorf("create temporary layer: %w", err)
	}
	tmpName := tmp.Name()
	closed := false
	published := false
	defer func() {
		if !closed {
			_ = tmp.Close()
		}
		if !published {
			_ = os.Remove(tmpName)
		}
	}()
	if err := tmp.Chmod(0o644); err != nil {
		return "", 0, fmt.Errorf("set layer mode: %w", err)
	}

	hasher := sha256.New()
	gw := gzip.NewWriter(io.MultiWriter(tmp, hasher))
	tw := tar.NewWriter(gw)

	walkErr := filepath.Walk(rootfsDir, func(path string, info os.FileInfo, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		rel, err := filepath.Rel(rootfsDir, path)
		if err != nil {
			return err
		}
		if rel == "." {
			return nil
		}

		linkTarget := ""
		if info.Mode()&os.ModeSymlink != 0 {
			linkTarget, err = os.Readlink(path)
			if err != nil {
				return fmt.Errorf("read symlink %q: %w", rel, err)
			}
		}
		header, err := tar.FileInfoHeader(info, linkTarget)
		if err != nil {
			return fmt.Errorf("create tar header %q: %w", rel, err)
		}
		header.Name = filepath.ToSlash(rel)
		if err := tw.WriteHeader(header); err != nil {
			return fmt.Errorf("write tar header %q: %w", rel, err)
		}

		if !info.Mode().IsRegular() {
			return nil
		}
		file, err := os.Open(path)
		if err != nil {
			return fmt.Errorf("open layer file %q: %w", rel, err)
		}
		_, copyErr := io.Copy(tw, file)
		closeErr := file.Close()
		if copyErr != nil {
			return fmt.Errorf("copy layer file %q: %w", rel, copyErr)
		}
		if closeErr != nil {
			return fmt.Errorf("close layer file %q: %w", rel, closeErr)
		}
		return nil
	})
	if walkErr != nil {
		_ = tw.Close()
		_ = gw.Close()
		return "", 0, fmt.Errorf("package layer tar.gz: %w", walkErr)
	}
	if err := tw.Close(); err != nil {
		_ = gw.Close()
		return "", 0, fmt.Errorf("finalize layer tar: %w", err)
	}
	if err := gw.Close(); err != nil {
		return "", 0, fmt.Errorf("finalize layer gzip: %w", err)
	}
	if err := tmp.Sync(); err != nil {
		return "", 0, fmt.Errorf("sync layer archive: %w", err)
	}
	if err := tmp.Close(); err != nil {
		return "", 0, fmt.Errorf("close layer archive: %w", err)
	}
	closed = true

	fi, err := os.Stat(tmpName)
	if err != nil {
		return "", 0, fmt.Errorf("stat completed layer: %w", err)
	}
	digest := "sha256:" + hex.EncodeToString(hasher.Sum(nil))
	if err := os.Rename(tmpName, destArchive); err != nil {
		return "", 0, fmt.Errorf("publish layer archive: %w", err)
	}
	published = true
	return digest, fi.Size(), nil
}

// BuildOCIManifest constructs OCI manifest JSON.
func BuildOCIManifest(layerDigest string, layerSize int64) (*OCIManifest, []byte, error) {
	configData := []byte(fmt.Sprintf(`{"created":%q,"architecture":"amd64","os":"linux"}`, time.Now().Format(time.RFC3339)))
	configHash := sha256.Sum256(configData)
	configDigest := "sha256:" + hex.EncodeToString(configHash[:])

	manifest := &OCIManifest{
		SchemaVersion: 2,
		MediaType:     "application/vnd.oci.image.manifest.v1+json",
		Config: OCIDescriptor{
			MediaType: "application/vnd.oci.image.config.v1+json",
			Digest:    configDigest,
			Size:      int64(len(configData)),
		},
		Layers: []OCIDescriptor{
			{
				MediaType: "application/vnd.oci.image.layer.v1.tar+gzip",
				Digest:    layerDigest,
				Size:      layerSize,
			},
		},
	}

	manifestBytes, err := json.MarshalIndent(manifest, "", "  ")
	if err != nil {
		return nil, nil, err
	}

	return manifest, manifestBytes, nil
}

// PushImage packages image into OCI format ready for registry dispatch.
func PushImage(st *state.Store, imageTag string, outputArchive string) error {
	if st == nil {
		return fmt.Errorf("state store is nil")
	}
	img, err := st.GetImage(imageTag)
	if err != nil {
		return fmt.Errorf("get image %q: %w", imageTag, err)
	}

	if img.RootFS == "" {
		return fmt.Errorf("image %q has empty rootfs", imageTag)
	}

	digest, sz, err := BuildOCILayer(img.RootFS, outputArchive)
	if err != nil {
		return fmt.Errorf("build layer: %w", err)
	}

	_, manifestBytes, err := BuildOCIManifest(digest, sz)
	if err != nil {
		return fmt.Errorf("build manifest: %w", err)
	}

	manifestPath := outputArchive + ".manifest.json"
	if err := atomicPublishFile(manifestPath, manifestBytes, 0o644); err != nil {
		return fmt.Errorf("publish manifest: %w", err)
	}
	return nil
}
