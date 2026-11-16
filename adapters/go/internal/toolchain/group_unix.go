//go:build unix

package toolchain

import (
	"os/exec"
	"syscall"
)

// ownGroup gives a program a process group of its own, and has a timeout kill the
// whole group: a block that starts other programs does not leave them running.
func ownGroup(cmd *exec.Cmd) {
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
	cmd.Cancel = func() error {
		return syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)
	}
}
