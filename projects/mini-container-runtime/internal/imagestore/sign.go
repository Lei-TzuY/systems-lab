package imagestore

import (
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"fmt"

	"minicontainer/internal/state"
)

// SignImage generates an HMAC-SHA256 signature for an image.
func SignImage(st *state.Store, nameOrID string, secretKey string) (string, error) {
	img, err := st.GetImage(nameOrID)
	if err != nil {
		return "", fmt.Errorf("get image %q: %w", nameOrID, err)
	}

	mac := hmac.New(sha256.New, []byte(secretKey))
	mac.Write([]byte(img.Name))
	mac.Write([]byte(img.Repository))
	mac.Write([]byte(img.Tag))
	mac.Write([]byte(fmt.Sprintf("%d", img.Size)))

	sig := hex.EncodeToString(mac.Sum(nil))
	return sig, nil
}

// VerifyImageSignature checks whether a given HMAC signature matches the image metadata.
func VerifyImageSignature(st *state.Store, nameOrID string, secretKey string, expectedSig string) (bool, error) {
	actualSig, err := SignImage(st, nameOrID, secretKey)
	if err != nil {
		return false, err
	}
	return hmac.Equal([]byte(actualSig), []byte(expectedSig)), nil
}
