//go:build !unix

package launcher

import (
	"errors"
	"os"
	"os/exec"
)

// Become runs the core and exits with its code: the nearest thing to replacing this
// process where there is no exec. It returns only when the core cannot be run.
func Become(core string, args, env []string) error {
	cmd := exec.Command(core, args...)
	cmd.Env, cmd.Stdin, cmd.Stdout, cmd.Stderr = env, os.Stdin, os.Stdout, os.Stderr
	err := cmd.Run()
	var exit *exec.ExitError
	if err == nil || errors.As(err, &exit) {
		os.Exit(cmd.ProcessState.ExitCode())
	}
	return err
}
