// Package toolchain runs programs for the adapter: the go command, and the programs it
// builds from attested code.
package toolchain

import (
	"bytes"
	"context"
	"errors"
	"os/exec"
	"time"
)

// maxOutput bounds what is kept of each stream of a program.
const maxOutput = 1 << 20

// Command is a program to run.
type Command struct {
	Path string
	Args []string
	// Dir is the working directory.
	Dir string
	// Timeout ends the program, and everything it started, when it is positive.
	Timeout time.Duration
}

// Outcome is how a program ended.
type Outcome struct {
	Stdout, Stderr string
	// Code is the exit status; -1 when the program did not exit by itself.
	Code     int
	TimedOut bool
	// Err is why the program could not be run at all.
	Err error
}

// capped keeps the first maxOutput bytes written to it and drops the rest.
type capped struct{ kept bytes.Buffer }

func (c *capped) Write(p []byte) (int, error) {
	if room := maxOutput - c.kept.Len(); room > 0 {
		c.kept.Write(p[:min(room, len(p))])
	}
	return len(p), nil
}

// Run runs a program to its end. Its output is kept and never reaches the adapter's own
// standard output, which carries only responses.
func Run(command Command) Outcome {
	ctx, cancel := context.Background(), context.CancelFunc(func() {})
	if command.Timeout > 0 {
		ctx, cancel = context.WithTimeout(ctx, command.Timeout)
	}
	defer cancel()
	cmd := exec.CommandContext(ctx, command.Path, command.Args...)
	cmd.Dir = command.Dir
	var stdout, stderr capped
	cmd.Stdout, cmd.Stderr = &stdout, &stderr
	ownGroup(cmd)
	cmd.WaitDelay = time.Second
	err := cmd.Run()
	outcome := Outcome{Stdout: stdout.kept.String(), Stderr: stderr.kept.String()}
	var exit *exec.ExitError
	switch {
	case ctx.Err() == context.DeadlineExceeded:
		outcome.Code, outcome.TimedOut = -1, true
	case errors.As(err, &exit):
		outcome.Code = exit.ExitCode()
	case err != nil:
		outcome.Code, outcome.Err = -1, err
	}
	return outcome
}
