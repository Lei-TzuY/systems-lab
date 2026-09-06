package imagestore

import (
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestImageSigningAndVerification(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	img := &state.Image{
		ID:         "img-sign-1",
		Repository: "ubuntu",
		Tag:        "22.04",
		Name:       "ubuntu:22.04",
		Size:       1000000,
		LoadedAt:   time.Now(),
	}
	_ = st.SaveImage(img)

	key := "my-secret-signing-key"
	sig, err := SignImage(st, "ubuntu:22.04", key)
	if err != nil || sig == "" {
		t.Fatalf("SignImage failed: %v, sig: %s", err, sig)
	}

	valid, err := VerifyImageSignature(st, "ubuntu:22.04", key, sig)
	if err != nil || !valid {
		t.Fatalf("VerifyImageSignature failed, expected valid signature")
	}

	invalid, _ := VerifyImageSignature(st, "ubuntu:22.04", "wrong-key", sig)
	if invalid {
		t.Fatalf("VerifyImageSignature with wrong key should fail")
	}
}
