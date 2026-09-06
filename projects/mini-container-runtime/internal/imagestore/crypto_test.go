package imagestore

import (
	"bytes"
	"testing"
)

func TestAES256GCMEncryption(t *testing.T) {
	plain := []byte("secret container layer data payload")
	key := "super-secure-passphrase-123"

	cipherBytes, err := EncryptPayload(plain, key)
	if err != nil {
		t.Fatalf("EncryptPayload error: %v", err)
	}

	decrypted, err := DecryptPayload(cipherBytes, key)
	if err != nil {
		t.Fatalf("DecryptPayload error: %v", err)
	}

	if !bytes.Equal(plain, decrypted) {
		t.Fatalf("Decrypted payload = %s, want %s", string(decrypted), string(plain))
	}

	_, errWrongKey := DecryptPayload(cipherBytes, "wrong-passphrase")
	if errWrongKey == nil {
		t.Fatalf("DecryptPayload with wrong passphrase should return error")
	}
}
