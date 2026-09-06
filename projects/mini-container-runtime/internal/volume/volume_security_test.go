package volume

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func setVolumeTestHome(t *testing.T) string {
	t.Helper()
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)
	return home
}

func TestVolumeRootSymlinkCannotCreateOrDeleteOutside(t *testing.T) {
	home := setVolumeTestHome(t)
	stateDir := filepath.Join(home, ".minicontainer")
	if err := os.MkdirAll(stateDir, 0o700); err != nil { t.Fatal(err) }
	outside := t.TempDir()
	victim := filepath.Join(outside, "victim")
	if err := os.MkdirAll(victim, 0o700); err != nil { t.Fatal(err) }
	sentinel := filepath.Join(victim, "sentinel")
	if err := os.WriteFile(sentinel, []byte("keep"), 0o600); err != nil { t.Fatal(err) }
	if err := os.Symlink(outside, filepath.Join(stateDir, "volumes")); err != nil { t.Fatal(err) }
	if _, err := CreateVolume("newvol"); err == nil || !strings.Contains(err.Error(), "volume storage directory is not a real directory") { t.Fatalf("CreateVolume symlink root error=%v", err) }
	if err := RemoveVolume("victim"); err == nil || !strings.Contains(err.Error(), "volume storage directory is not a real directory") { t.Fatalf("RemoveVolume symlink root error=%v", err) }
	if data, err := os.ReadFile(sentinel); err != nil || string(data) != "keep" { t.Fatalf("outside victim changed: data=%q err=%v", data, err) }
	if _, err := os.Stat(filepath.Join(outside, "newvol")); !os.IsNotExist(err) { t.Fatalf("CreateVolume wrote through symlinked root: %v", err) }
}

func TestVolumeStateBaseSymlinkRejected(t *testing.T) {
	home := setVolumeTestHome(t)
	outside := t.TempDir()
	if err := os.Symlink(outside, filepath.Join(home, ".minicontainer")); err != nil { t.Fatal(err) }
	if _, err := CreateVolume("db"); err == nil || !strings.Contains(err.Error(), "state directory is not a real directory") { t.Fatalf("CreateVolume symlink state base error=%v", err) }
	if _, err := os.Stat(filepath.Join(outside, "volumes", "db")); !os.IsNotExist(err) { t.Fatalf("symlinked state base received volume data: %v", err) }
}

func TestCreateVolumeRejectsSymlinkedVolumeDirectory(t *testing.T) {
	setVolumeTestHome(t)
	root, err := volumeRoot(true); if err != nil { t.Fatal(err) }
	outside := t.TempDir()
	if err := os.Symlink(outside, filepath.Join(root, "db")); err != nil { t.Fatal(err) }
	if _, err := CreateVolume("db"); err == nil || !strings.Contains(err.Error(), "not a real directory") { t.Fatalf("CreateVolume symlink dir error=%v", err) }
	if _, err := os.Stat(filepath.Join(outside, "volume.json")); !os.IsNotExist(err) { t.Fatalf("symlinked volume dir received metadata: %v", err) }
}

func TestCreateVolumeRejectsSymlinkedDataDirectory(t *testing.T) {
	setVolumeTestHome(t)
	root, err := volumeRoot(true); if err != nil { t.Fatal(err) }
	volDir := filepath.Join(root, "db"); if err := os.Mkdir(volDir, 0o700); err != nil { t.Fatal(err) }
	outside := t.TempDir()
	if err := os.Symlink(outside, filepath.Join(volDir, "_data")); err != nil { t.Fatal(err) }
	if _, err := CreateVolume("db"); err == nil || !strings.Contains(err.Error(), "data directory") { t.Fatalf("CreateVolume symlink data error=%v", err) }
	if _, err := os.Stat(filepath.Join(volDir, "volume.json")); !os.IsNotExist(err) { t.Fatalf("metadata written despite symlink data dir: %v", err) }
}

func TestCreateVolumeRejectsSymlinkedMetadata(t *testing.T) {
	setVolumeTestHome(t)
	root, err := volumeRoot(true); if err != nil { t.Fatal(err) }
	volDir, _, err := ensureVolumeLayout(root, "db", true); if err != nil { t.Fatal(err) }
	outside := filepath.Join(t.TempDir(), "outside.json"); const sentinel = "do-not-overwrite"
	if err := os.WriteFile(outside, []byte(sentinel), 0o600); err != nil { t.Fatal(err) }
	if err := os.Symlink(outside, filepath.Join(volDir, "volume.json")); err != nil { t.Fatal(err) }
	if _, err := CreateVolume("db"); err == nil || !strings.Contains(err.Error(), "metadata is not a regular file") { t.Fatalf("CreateVolume symlink metadata error=%v", err) }
	if data, err := os.ReadFile(outside); err != nil || string(data) != sentinel { t.Fatalf("metadata symlink target changed: data=%q err=%v", data, err) }
}

func TestGetAndRemoveVolumeRejectTamperedMountPath(t *testing.T) {
	setVolumeTestHome(t)
	vol, err := CreateVolume("db"); if err != nil { t.Fatal(err) }
	sentinel := filepath.Join(vol.MountPath, "sentinel"); if err := os.WriteFile(sentinel, []byte("keep"), 0o600); err != nil { t.Fatal(err) }
	outside := t.TempDir()
	tampered := &Volume{Name: "db", MountPath: outside, CreatedAt: time.Now()}
	data, err := json.Marshal(tampered); if err != nil { t.Fatal(err) }
	metaPath := filepath.Join(DefaultVolumeDir(), "db", "volume.json")
	if err := os.WriteFile(metaPath, data, 0o600); err != nil { t.Fatal(err) }
	if _, err := GetVolume("db"); err == nil || !strings.Contains(err.Error(), "does not match managed data path") { t.Fatalf("GetVolume tampered mount path error=%v", err) }
	if got := ResolveVolumePath("db"); got != "db" { t.Fatalf("ResolveVolumePath trusted tampered mount path: %q", got) }
	if err := RemoveVolume("db"); err == nil || !strings.Contains(err.Error(), "validate volume before removal") { t.Fatalf("RemoveVolume tampered metadata error=%v", err) }
	if data, err := os.ReadFile(sentinel); err != nil || string(data) != "keep" { t.Fatalf("volume data changed after rejected removal: data=%q err=%v", data, err) }
}

func TestVolumeStoragePrivateMetadataAndUpdate(t *testing.T) {
	setVolumeTestHome(t)
	vol, err := CreateVolume("db0"); if err != nil { t.Fatal(err) }
	keep := filepath.Join(vol.MountPath, "keep"); if err := os.WriteFile(keep, []byte("data"), 0o600); err != nil { t.Fatal(err) }
	if _, err := CreateVolume("db0"); err != nil { t.Fatalf("recreate volume: %v", err) }
	if data, err := os.ReadFile(keep); err != nil || string(data) != "data" { t.Fatalf("recreate destroyed volume data: data=%q err=%v", data, err) }
	rootInfo, err := os.Stat(DefaultVolumeDir()); if err != nil { t.Fatal(err) }; if rootInfo.Mode().Perm() != 0o700 { t.Fatalf("volume root mode=%#o", rootInfo.Mode().Perm()) }
	volDir := filepath.Join(DefaultVolumeDir(), "db0")
	vInfo, err := os.Stat(volDir); if err != nil { t.Fatal(err) }; if vInfo.Mode().Perm() != 0o700 { t.Fatalf("volume dir mode=%#o", vInfo.Mode().Perm()) }
	mInfo, err := os.Stat(filepath.Join(volDir, "volume.json")); if err != nil { t.Fatal(err) }; if mInfo.Mode().Perm() != 0o600 { t.Fatalf("metadata mode=%#o", mInfo.Mode().Perm()) }
	dInfo, err := os.Stat(filepath.Join(volDir, "_data")); if err != nil { t.Fatal(err) }; if dInfo.Mode().Perm() != 0o755 { t.Fatalf("data dir mode=%#o", dInfo.Mode().Perm()) }
}
