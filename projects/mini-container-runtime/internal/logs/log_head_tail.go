// Package logs provides container log processing utilities.
// This file implements head/tail line selectors for extracting the first N
// or last N lines from container log streams, analogous to Unix head/tail commands.

package logs

// HeadLines returns the first n lines from a log stream.
// If n <= 0 or n >= len(lines), all lines are returned.
func HeadLines(lines []string, n int) []string {
	if n <= 0 || n >= len(lines) {
		return lines
	}
	out := make([]string, n)
	copy(out, lines[:n])
	return out
}

// TailLines returns the last n lines from a log stream.
// If n <= 0 or n >= len(lines), all lines are returned.
func TailLines(lines []string, n int) []string {
	if n <= 0 || n >= len(lines) {
		return lines
	}
	start := len(lines) - n
	out := make([]string, n)
	copy(out, lines[start:])
	return out
}

// HeadTailLines returns the first headN lines and the last tailN lines,
// with a separator marker between them if the stream is longer than headN+tailN.
func HeadTailLines(lines []string, headN, tailN int) []string {
	total := len(lines)
	if headN <= 0 {
		headN = 0
	}
	if tailN <= 0 {
		tailN = 0
	}

	if headN+tailN >= total {
		return lines
	}

	out := make([]string, 0, headN+1+tailN)
	out = append(out, lines[:headN]...)
	skipped := total - headN - tailN
	if skipped > 0 {
		out = append(out, "--- (skipped "+itoa(skipped)+" lines) ---")
	}
	out = append(out, lines[total-tailN:]...)
	return out
}

func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	s := ""
	neg := false
	if n < 0 {
		neg = true
		n = -n
	}
	for n > 0 {
		s = string(rune('0'+n%10)) + s
		n /= 10
	}
	if neg {
		s = "-" + s
	}
	return s
}
