package imagestore

import (
	"testing"
)

func TestExtractDomainname(t *testing.T) {
	tests := []struct {
		name     string
		json     string
		expected string
		wantErr  bool
	}{
		{
			name:     "domain name set",
			json:     `{"config":{"Domainname":"example.com"}}`,
			expected: "example.com",
		},
		{
			name:     "domain name empty",
			json:     `{"config":{"Domainname":""}}`,
			expected: "",
		},
		{
			name:     "field missing",
			json:     `{"config":{}}`,
			expected: "",
		},
		{
			name:     "subdomain",
			json:     `{"config":{"Domainname":"app.internal.corp"}}`,
			expected: "app.internal.corp",
		},
		{
			name:    "invalid json",
			json:    `{invalid`,
			wantErr: true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got, err := ExtractDomainname([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if got != tc.expected {
				t.Errorf("ExtractDomainname() = %q, want %q", got, tc.expected)
			}
		})
	}
}

func TestFormatDomainname(t *testing.T) {
	t.Run("set", func(t *testing.T) {
		got := FormatDomainname([]byte(`{"config":{"Domainname":"example.com"}}`))
		if got != "Domainname: example.com" {
			t.Errorf("got %q", got)
		}
	})

	t.Run("not set", func(t *testing.T) {
		got := FormatDomainname([]byte(`{"config":{}}`))
		if got != "(not set)" {
			t.Errorf("got %q", got)
		}
	})

	t.Run("invalid json", func(t *testing.T) {
		got := FormatDomainname([]byte(`{bad`))
		if got == "" || got == "(not set)" {
			t.Errorf("expected error message, got %q", got)
		}
	})
}
