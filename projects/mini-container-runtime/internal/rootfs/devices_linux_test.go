//go:build linux

package rootfs

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"syscall"
	"testing"
)

type recordedMount struct {
	source string
	target string
	fstype string
	flags  uintptr
	data   string
}

func permissiveDeviceOps(t *testing.T, mounts *[]recordedMount, unmounts *[]string, created *[]string, links map[string]string, validated *[]string) deviceMountOps {
	t.Helper()
	return deviceMountOps{
		ensureDir: func(path string, mode os.FileMode) error { return nil },
		unmount: func(path string, flags int) error {
			if flags != syscall.MNT_DETACH {
				t.Fatalf("unmount flags=%d, want MNT_DETACH", flags)
			}
			*unmounts = append(*unmounts, path)
			return nil
		},
		mount: func(source, target, fstype string, flags uintptr, data string) error {
			*mounts = append(*mounts, recordedMount{source: source, target: target, fstype: fstype, flags: flags, data: data})
			return nil
		},
		createFile: func(path string, mode os.FileMode) error {
			if mode != 0o666 {
				t.Fatalf("device target %s mode=%#o, want 0666", path, mode)
			}
			*created = append(*created, path)
			return nil
		},
		symlink: func(oldname, newname string) error {
			links[newname] = oldname
			return nil
		},
		validateSource: func(path string) error {
			*validated = append(*validated, path)
			return nil
		},
	}
}

func TestPreparePrivateDevicesUsesAllowlistOnly(t *testing.T) {
	const root = "/container/root"
	dev := filepath.Join(root, "dev")
	var mounts []recordedMount
	var unmounts, created, validated []string
	links := make(map[string]string)
	ops := permissiveDeviceOps(t, &mounts, &unmounts, &created, links, &validated)

	if err := preparePrivateDevicesWithOps(root, false, ops); err != nil {
		t.Fatal(err)
	}
	if len(unmounts) != 1 || unmounts[0] != dev {
		t.Fatalf("unmounts=%v, want [%s]", unmounts, dev)
	}
	if len(mounts) == 0 {
		t.Fatal("no mounts recorded")
	}
	first := mounts[0]
	if first.source != "tmpfs" || first.target != dev || first.fstype != "tmpfs" {
		t.Fatalf("first mount=%+v, want private /dev tmpfs", first)
	}
	if first.flags&syscall.MS_NODEV != 0 {
		t.Fatal("private /dev tmpfs was mounted nodev; allowlisted character devices would be unusable")
	}

	wantDevices := make(map[string]bool, len(safeDeviceNames))
	for _, name := range safeDeviceNames {
		wantDevices[filepath.Join("/dev", name)] = true
	}
	if len(validated) != len(wantDevices) {
		t.Fatalf("validated=%v, want %d safe devices", validated, len(wantDevices))
	}
	for _, source := range validated {
		if !wantDevices[source] {
			t.Fatalf("unexpected validated device %q", source)
		}
	}

	binds := make(map[string]string)
	for _, mount := range mounts {
		if mount.flags&syscall.MS_BIND == 0 {
			continue
		}
		if mount.source == "/dev" {
			t.Fatal("recursive host /dev bind survived private-device setup")
		}
		binds[mount.source] = mount.target
	}
	if len(binds) != len(wantDevices) {
		t.Fatalf("device binds=%v, want exactly %d allowlisted binds", binds, len(wantDevices))
	}
	for source := range wantDevices {
		name := filepath.Base(source)
		if binds[source] != filepath.Join(dev, name) {
			t.Fatalf("bind %s -> %q, want %q", source, binds[source], filepath.Join(dev, name))
		}
	}
	if len(created) != len(wantDevices) {
		t.Fatalf("created device targets=%v", created)
	}

	var sawDevpts, sawShm bool
	for _, mount := range mounts {
		if mount.fstype == "devpts" {
			sawDevpts = mount.target == filepath.Join(dev, "pts") && strings.Contains(mount.data, "newinstance")
		}
		if mount.source == "tmpfs" && mount.target == filepath.Join(dev, "shm") {
			sawShm = mount.flags&syscall.MS_NODEV != 0 && strings.Contains(mount.data, "mode=1777")
		}
	}
	if !sawDevpts {
		t.Fatal("private devpts mount missing")
	}
	if !sawShm {
		t.Fatal("private /dev/shm mount missing or insufficiently restricted")
	}

	wantLinks := map[string]string{
		filepath.Join(dev, "ptmx"):   "pts/ptmx",
		filepath.Join(dev, "fd"):     "/proc/self/fd",
		filepath.Join(dev, "stdin"):  "/proc/self/fd/0",
		filepath.Join(dev, "stdout"): "/proc/self/fd/1",
		filepath.Join(dev, "stderr"): "/proc/self/fd/2",
	}
	if len(links) != len(wantLinks) {
		t.Fatalf("links=%v, want %v", links, wantLinks)
	}
	for path, target := range wantLinks {
		if links[path] != target {
			t.Fatalf("link %s -> %q, want %q", path, links[path], target)
		}
	}
}

func TestPreparePrivateDevicesFailsClosedOnDetachFailure(t *testing.T) {
	var mounts []recordedMount
	var unmounts, created, validated []string
	links := make(map[string]string)
	ops := permissiveDeviceOps(t, &mounts, &unmounts, &created, links, &validated)
	cause := syscall.EPERM
	ops.unmount = func(path string, flags int) error { return cause }

	err := preparePrivateDevicesWithOps("/container/root", false, ops)
	if !errors.Is(err, cause) {
		t.Fatalf("detach error=%v, want EPERM preserved", err)
	}
	if len(mounts) != 0 {
		t.Fatalf("mounted after inherited /dev detach failed: %+v", mounts)
	}
}

func TestPreparePrivateDevicesToleratesNoInheritedMount(t *testing.T) {
	var mounts []recordedMount
	var unmounts, created, validated []string
	links := make(map[string]string)
	ops := permissiveDeviceOps(t, &mounts, &unmounts, &created, links, &validated)
	ops.unmount = func(path string, flags int) error { return syscall.EINVAL }
	if err := preparePrivateDevicesWithOps("/container/root", false, ops); err != nil {
		t.Fatalf("EINVAL for non-mount /dev should be tolerated: %v", err)
	}
	if len(mounts) == 0 || mounts[0].source != "tmpfs" {
		t.Fatalf("private /dev not created after EINVAL: %+v", mounts)
	}
}

func TestSecureEnsureDeviceDirRejectsSymlink(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	dev := filepath.Join(root, "dev")
	if err := os.Symlink(outside, dev); err != nil {
		t.Fatal(err)
	}
	if err := secureEnsureDeviceDir(dev, 0o755); err == nil {
		t.Fatal("symlink /dev mountpoint accepted")
	}
}

func TestDefaultDeviceSourceValidationRejectsNonDeviceAndSymlink(t *testing.T) {
	ops := defaultDeviceMountOps()
	regular := filepath.Join(t.TempDir(), "regular")
	if err := os.WriteFile(regular, []byte("x"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := ops.validateSource(regular); err == nil {
		t.Fatal("regular file accepted as safe device source")
	}
	link := regular + ".link"
	if err := os.Symlink(regular, link); err != nil {
		t.Fatal(err)
	}
	if err := ops.validateSource(link); err == nil {
		t.Fatal("symlink accepted as safe device source")
	}
}

func TestIsolateRequiresDeviceSetupBeforePivot(t *testing.T) {
	cause := errors.New("device setup failed")
	pivotCalls := 0
	err := isolateWithDeviceSetup("/fake/root", false,
		func(newRoot string, debug bool) error { return cause },
		func(newRoot string, debug bool) error {
			pivotCalls++
			return nil
		},
	)
	if !errors.Is(err, cause) || !strings.Contains(err.Error(), "private /dev isolation required") {
		t.Fatalf("device failure not preserved as isolation failure: %v", err)
	}
	if pivotCalls != 0 {
		t.Fatalf("pivot called %d times after device setup failure", pivotCalls)
	}

	order := make([]string, 0, 2)
	err = isolateWithDeviceSetup("/fake/root", true,
		func(newRoot string, debug bool) error {
			order = append(order, "devices")
			return nil
		},
		func(newRoot string, debug bool) error {
			order = append(order, "pivot")
			return nil
		},
	)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Join(order, ",") != "devices,pivot" {
		t.Fatalf("isolation order=%v, want devices before pivot", order)
	}
}

func TestIsolateRejectsNilDeviceSetup(t *testing.T) {
	err := isolateWithDeviceSetup("/fake/root", false, nil, func(string, bool) error { return nil })
	if err == nil || !strings.Contains(err.Error(), "private /dev isolation function is nil") {
		t.Fatalf("nil device setup error=%v", err)
	}
}
