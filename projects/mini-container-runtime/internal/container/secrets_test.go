package container

import (
	"testing"
)

func TestMaskEnvVars(t *testing.T) {
	env := []string{
		"PORT=8080",
		"DB_PASSWORD=supersecret",
		"API_KEY=abc123xyz",
		"NODE_ENV=production",
	}

	masked := MaskEnvVars(env)

	if masked[0] != "PORT=8080" {
		t.Fatalf("PORT should not be masked: %s", masked[0])
	}
	if masked[1] != "DB_PASSWORD=******" {
		t.Fatalf("DB_PASSWORD should be masked: %s", masked[1])
	}
	if masked[2] != "API_KEY=******" {
		t.Fatalf("API_KEY should be masked: %s", masked[2])
	}
	if masked[3] != "NODE_ENV=production" {
		t.Fatalf("NODE_ENV should not be masked: %s", masked[3])
	}
}
