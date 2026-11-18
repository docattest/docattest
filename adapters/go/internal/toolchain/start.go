package toolchain

import "bytes"

// startWatch passes a program's standard error on, all but the line that says the
// program's own code begins, and closes `started` when that line has been written.
type startWatch struct {
	line    []byte
	out     *capped
	started chan struct{}
	// held is the end of what was written so far, kept back because it might be the
	// beginning of the line, which can arrive in pieces.
	held []byte
	seen bool
}

func newStartWatch(line string, out *capped) *startWatch {
	return &startWatch{line: []byte(line), out: out, started: make(chan struct{})}
}

// Write passes on what cannot be part of the start line.
func (w *startWatch) Write(p []byte) (int, error) {
	if w.seen {
		return w.out.Write(p)
	}
	w.held = append(w.held, p...)
	if at := bytes.Index(w.held, w.line); at >= 0 {
		w.seen = true
		close(w.started)
		before, after := w.held[:at], w.held[at+len(w.line):]
		w.held = nil
		w.out.Write(before)
		w.out.Write(after)
		return len(p), nil
	}
	cut := len(w.held) - min(len(w.held), len(w.line)-1)
	passed := w.held[:cut]
	w.held = append([]byte(nil), w.held[cut:]...)
	w.out.Write(passed)
	return len(p), nil
}

// flush passes on what is still held back, once the program has ended.
func (w *startWatch) flush() {
	w.out.Write(w.held)
}
