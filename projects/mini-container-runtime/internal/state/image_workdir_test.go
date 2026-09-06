package state

import "testing"

func TestImageWorkingDirSurvivesImageRepublish(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open() error = %v", err)
	}
	defer st.Close()

	const name = "example:latest"
	if err := st.SaveImageWorkingDir(name, "/srv/app"); err != nil {
		t.Fatalf("SaveImageWorkingDir() error = %v", err)
	}
	if err := st.SaveImage(&Image{Name: name, RootFS: t.TempDir()}); err != nil {
		t.Fatalf("SaveImage() error = %v", err)
	}

	got, ok, err := st.ImageWorkingDir(name)
	if err != nil {
		t.Fatalf("ImageWorkingDir() error = %v", err)
	}
	if !ok || got != "/srv/app" {
		t.Fatalf("ImageWorkingDir() = %q, %v; want /srv/app, true", got, ok)
	}
}
