# The sandbox

A sandboxed command's environment adds these variables to its parent's: <!-- fact @sandbox: sessionEnv -->
- `LANG=C.UTF-8`
- `TZ=UTC`

Their names: <!-- fact @sandbox: SessionEnvNames() -->
- `LANG`
- `TZ`

A command whose parent has no environment has `2` variables. <!-- fact @sandbox: len(Env(nil)) -->

It is refused these capabilities: <!-- fact @sandbox: ForbiddenCapNames() -->
- `CAP_SYS_ADMIN`
- `CAP_NET_RAW`

Go prints them as `[{CAP_SYS_ADMIN mounts filesystems} {CAP_NET_RAW forges packets}]`. <!-- fact @sandbox: forbiddenCaps -->

Each for a reason: <!-- fact @sandbox: func() (reasons []string) { for _, c := range forbiddenCaps { reasons = append(reasons, c.Reason) }; return }() -->
- mounts filesystems
- forges packets

Asked about any other capability, such as CAP_CHOWN, the sandbox says `false`. <!-- fact @sandbox: Refuses("CAP_CHOWN") -->

Its environment names its shell: `SHELL=/bin/sh`. <!-- fact @sandbox: ShellEnv[0] -->

It may hold `64` files open. <!-- fact @sandbox: limits["nofile"] -->
