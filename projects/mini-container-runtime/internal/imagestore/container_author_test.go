package imagestore

import (
	"testing"
)

func TestExtractAuthorInfo(t *testing.T) {
	tests := []struct {
		name           string
		json           string
		wantAuthor     string
		wantMaintainer string
		wantErr        bool
	}{
		{
			name:           "author and maintainer label",
			json:           `{"author":"John Doe","config":{"Labels":{"maintainer":"ops@example.com"}}}`,
			wantAuthor:     "John Doe",
			wantMaintainer: "ops@example.com",
		},
		{
			name:           "only author",
			json:           `{"author":"Jane Smith"}`,
			wantAuthor:     "Jane Smith",
			wantMaintainer: "",
		},
		{
			name:           "fallback to config.Author",
			json:           `{"config":{"Author":"Config Author"}}`,
			wantAuthor:     "Config Author",
			wantMaintainer: "",
		},
		{
			name:           "empty config",
			json:           `{"config":{}}`,
			wantAuthor:     "",
			wantMaintainer: "",
		},
		{
			name:    "invalid json",
			json:    `{invalid`,
			wantErr: true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			info, err := ExtractAuthorInfo([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if info.Author != tc.wantAuthor || info.Maintainer != tc.wantMaintainer {
				t.Errorf("got Author=%q Maintainer=%q, want %q %q",
					info.Author, info.Maintainer, tc.wantAuthor, tc.wantMaintainer)
			}
		})
	}
}

func TestFormatAuthorInfo(t *testing.T) {
	t.Run("with both", func(t *testing.T) {
		got := FormatAuthorInfo([]byte(`{"author":"Dev","config":{"Labels":{"maintainer":"Ops"}}}`))
		want := "Author: Dev, Maintainer: Ops"
		if got != want {
			t.Errorf("got %q, want %q", got, want)
		}
	})

	t.Run("unknown", func(t *testing.T) {
		got := FormatAuthorInfo([]byte(`{}`))
		if got != "Author: (unknown)" {
			t.Errorf("got %q", got)
		}
	})
}
