package network

import (
	"testing"
)

func TestGetInterfaceStats(t *testing.T) {
	st, err := GetInterfaceStats("veth-test0")
	if err != nil {
		t.Fatalf("GetInterfaceStats error: %v", err)
	}
	if st.RxBytes == 0 || st.TxBytes == 0 {
		t.Fatalf("NetStats empty: %+v", st)
	}
}
