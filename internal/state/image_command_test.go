package state

import (
	"reflect"
	"testing"
	"time"
)

func TestImageCommandSurvivesBasicImageRepublish(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open() error = %v", err)
	}
	defer st.Close()

	const name = "example:latest"
	rootfs := t.TempDir()
	if err := st.SaveImage(&Image{Name: name, RootFS: rootfs, LoadedAt: time.Now()}); err != nil {
		t.Fatalf("SaveImage(initial) error = %v", err)
	}
	want := ImageCommand{Entrypoint: []string{"/bin/app"}, Cmd: []string{"serve", "--port=8080"}}
	if err := st.SaveImageCommand(name, want); err != nil {
		t.Fatalf("SaveImageCommand() error = %v", err)
	}

	// cmdPull historically republishes only basic image metadata after PullImage.
	if err := st.SaveImage(&Image{Name: name, RootFS: rootfs, LoadedAt: time.Now()}); err != nil {
		t.Fatalf("SaveImage(republish) error = %v", err)
	}

	got, ok, err := st.ImageCommandConfig(name)
	if err != nil {
		t.Fatalf("ImageCommandConfig() error = %v", err)
	}
	if !ok {
		t.Fatal("ImageCommandConfig() ok = false, want true")
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("ImageCommandConfig() = %#v, want %#v", got, want)
	}
}
