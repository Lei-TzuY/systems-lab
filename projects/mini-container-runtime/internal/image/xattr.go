package image

import (
	"archive/tar"
	"strings"
)

const paxSchilyXattrPrefix = "SCHILY.xattr."

func tarXattrsPortable(hdr *tar.Header) map[string][]byte {
	if hdr == nil || len(hdr.PAXRecords) == 0 {
		return nil
	}
	out := make(map[string][]byte)
	for key, value := range hdr.PAXRecords {
		if !strings.HasPrefix(key, paxSchilyXattrPrefix) {
			continue
		}
		name := strings.TrimPrefix(key, paxSchilyXattrPrefix)
		if name == "" {
			continue
		}
		out[name] = append([]byte(nil), []byte(value)...)
	}
	if len(out) == 0 {
		return nil
	}
	return out
}
