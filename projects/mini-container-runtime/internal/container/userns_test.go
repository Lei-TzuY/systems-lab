package container

import (
	"testing"
)

func TestFormatIDMap(t *testing.T) {
	maps := []IDMap{
		{ContainerID: 0, HostID: 100000, Count: 65536},
	}

	formatted := FormatIDMap(maps)
	expected := "0 100000 65536"
	if formatted != expected {
		t.Fatalf("FormatIDMap = %q, want %q", formatted, expected)
	}

	if err := ApplyUserNSMappings(-1, maps, maps); err != nil {
		t.Fatalf("ApplyUserNSMappings with negative pid should return nil gracefully on non-linux")
	}
}
