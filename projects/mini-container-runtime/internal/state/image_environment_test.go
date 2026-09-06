package state

import (
	"reflect"
	"testing"
	"time"
)

func TestImageEnvironmentSurvivesBasicImageMetadataRepublish(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open() error = %v", err)
	}
	defer st.Close()

	const name = "example:latest"
	rootfs := t.TempDir()
	if err := st.SaveImage(&Image{Name: name, RootFS: rootfs, LoadedAt: time.Now(), Env: []string{"FROM_IMAGE=present"}}); err != nil {
		t.Fatalf("SaveImage(initial) error = %v", err)
	}
	if err := st.SaveImageEnvironment(name, []string{"FROM_IMAGE=present"}); err != nil {
		t.Fatalf("SaveImageEnvironment() error = %v", err)
	}

	// cmdPull historically republishes basic image metadata after registry.PullImage.
	if err := st.SaveImage(&Image{Name: name, RootFS: rootfs, LoadedAt: time.Now()}); err != nil {
		t.Fatalf("SaveImage(republish) error = %v", err)
	}

	got, ok, err := st.ImageEnvironment(name)
	if err != nil {
		t.Fatalf("ImageEnvironment() error = %v", err)
	}
	if !ok {
		t.Fatal("ImageEnvironment() lost durable environment after metadata republish")
	}
	want := []string{"FROM_IMAGE=present"}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("ImageEnvironment() = %#v, want %#v", got, want)
	}
}
