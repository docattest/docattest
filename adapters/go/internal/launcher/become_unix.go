//go:build unix

package launcher

import "syscall"

// Become replaces this process with the core: arguments, standard streams, signals and
// the exit code are then the core's own. It returns only when the core cannot be run.
func Become(core string, args, env []string) error {
	return syscall.Exec(core, append([]string{core}, args...), env)
}
