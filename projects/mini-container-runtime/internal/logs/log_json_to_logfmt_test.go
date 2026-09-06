package logs

import (
	"reflect"
	"strings"
	"testing"
)

func TestJSONToLogfmt(t *testing.T) {
	jsonLine := `{"level":"info","msg":"server started","port":8080,"secure":true}`
	got := JSONToLogfmt(jsonLine)

	expectedKeywords := []string{
		`level=info`,
		`msg="server started"`,
		`port=8080`,
		`secure=true`,
	}

	for _, kw := range expectedKeywords {
		if !strings.Contains(got, kw) {
			t.Errorf("expected %q in logfmt output %q", kw, got)
		}
	}
}

func TestJSONToLogfmt_NestedObjectFlattening(t *testing.T) {
	jsonLine := `{"level":"error","http":{"method":"POST","status":500,"path":"/api/v1/run"}}`
	got := JSONToLogfmt(jsonLine)

	expectedKeywords := []string{
		`http.method=POST`,
		`http.path=/api/v1/run`,
		`http.status=500`,
		`level=error`,
	}

	for _, kw := range expectedKeywords {
		if !strings.Contains(got, kw) {
			t.Errorf("expected %q in flattened logfmt %q", kw, got)
		}
	}
}

func TestJSONToLogfmt_Arrays(t *testing.T) {
	jsonLine := `{"tags":["prod","us-east"]}`
	got := JSONToLogfmt(jsonLine)

	if got != `tags="prod,us-east"` {
		t.Errorf("got %q, want 'tags=\"prod,us-east\"'", got)
	}
}

func TestJSONToLogfmt_NonJSON(t *testing.T) {
	plain := "plain text log line"
	if got := JSONToLogfmt(plain); got != plain {
		t.Errorf("got %q, want %q", got, plain)
	}
}

func TestConvertJSONStreamToLogfmt(t *testing.T) {
	lines := []string{
		`{"msg":"start"}`,
		"plain line",
	}
	got := ConvertJSONStreamToLogfmt(lines)
	want := []string{
		"msg=start",
		"plain line",
	}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("got %#v, want %#v", got, want)
	}
}
