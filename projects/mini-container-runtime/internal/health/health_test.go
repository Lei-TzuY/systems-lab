package health

import "testing"

func TestEvaluate(t *testing.T) {
	tests := []struct {
		failures int
		retries  int
		want     string
	}{
		{failures: 0, retries: 3, want: StatusHealthy},
		{failures: 1, retries: 3, want: StatusStarting},
		{failures: 2, retries: 3, want: StatusStarting},
		{failures: 3, retries: 3, want: StatusUnhealthy},
		{failures: 5, retries: 3, want: StatusUnhealthy},
	}

	for _, tt := range tests {
		t.Run(tt.want, func(t *testing.T) {
			got := Evaluate(tt.failures, tt.retries)
			if got != tt.want {
				t.Errorf("Evaluate(%d, %d) = %q, want %q", tt.failures, tt.retries, got, tt.want)
			}
		})
	}
}
