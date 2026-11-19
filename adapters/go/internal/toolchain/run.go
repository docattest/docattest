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
	// Env is what the program's environment adds to this process's.
	Env []string
	// Timeout ends the program, and everything it started, when it is positive.
	Timeout time.Duration
	// Start, when given, has Timeout count from the moment the program says its own code
	// begins, rather than from its launch.
	Start *Start
}

// Start is how a program says that its own code begins: everything before that, the
// system's start-up of the program among it, is not counted against its Timeout.
type Start struct {
	// Line is what the program writes to its standard error then. It is not kept in the
	// Outcome's Stderr.
	Line string
	// Limit ends a program that has not written Line this long after its launch.
	Limit time.Duration
}

// Outcome is how a program ended.
type Outcome struct {
	Stdout, Stderr string
	// Code is the exit status; -1 when the program did not exit by itself.
	Code     int
	TimedOut bool
	// NeverStarted says the program was ended before it wrote its Start line.
	NeverStarted bool
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

// The causes Run ends a program for.
var (
	errTimedOut     = errors.New("the program ran out of time")
	errNeverStarted = errors.New("the program did not start")
)

// Run runs a program to its end. Its output is kept and never reaches the adapter's own
// standard output, which carries only responses.
func Run(command Command) Outcome {
	ctx, end := context.WithCancelCause(context.Background())
	defer end(nil)
	cmd := exec.CommandContext(ctx, command.Path, command.Args...)
	cmd.Dir = command.Dir
	if len(command.Env) > 0 {
		// Environ, and not os.Environ, keeps the PWD that exec gives the program's Dir: a
		// go command whose PWD is another directory takes the path of its working directory
		// with every symbolic link resolved, which is not the path its overlay names.
		cmd.Env = append(cmd.Environ(), command.Env...)
	}
	var stdout, stderr capped
	cmd.Stdout, cmd.Stderr = &stdout, &stderr
	// Without a start there is nothing to watch for, and `started` is nil: never closed.
	var watch *startWatch
	var started <-chan struct{}
	if command.Start != nil {
		watch = newStartWatch(command.Start.Line, &stderr)
		cmd.Stderr, started = watch, watch.started
	}
	ownGroup(cmd)
	cmd.WaitDelay = time.Second
	if err := cmd.Start(); err != nil {
		return Outcome{Code: -1, Err: err}
	}
	exited := make(chan struct{})
	go enforce(command, started, exited, end)
	err := cmd.Wait()
	close(exited)
	if watch != nil {
		watch.flush()
	}
	return outcome(err, context.Cause(ctx), stdout.kept.String(), stderr.kept.String())
}

// enforce ends the program, with the cause, when it has not started or has not finished
// in time. It returns once the program exits.
func enforce(command Command, started, exited <-chan struct{}, end context.CancelCauseFunc) {
	if command.Start != nil {
		switch wait(command.Start.Limit, started, exited) {
		case exitedFirst:
			return
		case expired:
			end(errNeverStarted)
			return
		}
	}
	// Nothing is awaited but the exit: receiving from a nil channel never happens.
	if command.Timeout > 0 && wait(command.Timeout, nil, exited) == expired {
		end(errTimedOut)
	}
}

// waited is what came first of what `wait` waits for.
type waited int

const (
	arrived waited = iota
	exitedFirst
	expired
)

// wait waits up to `limit` for `awaited`, or for the program to exit.
func wait(limit time.Duration, awaited, exited <-chan struct{}) waited {
	timer := time.NewTimer(limit)
	defer timer.Stop()
	select {
	case <-awaited:
		return arrived
	case <-exited:
		return exitedFirst
	case <-timer.C:
		return expired
	}
}

// outcome is how a program ended, from what Wait returned and why it was ended, if it was.
func outcome(err, cause error, stdout, stderr string) Outcome {
	done := Outcome{Stdout: stdout, Stderr: stderr}
	var exit *exec.ExitError
	switch {
	case cause == errTimedOut:
		done.Code, done.TimedOut = -1, true
	case cause == errNeverStarted:
		done.Code, done.NeverStarted = -1, true
	case errors.As(err, &exit):
		done.Code = exit.ExitCode()
	case err != nil:
		done.Code, done.Err = -1, err
	}
	return done
}
