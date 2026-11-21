# Coding agents

Everything docattest knows about coding agents is in this directory. No source file of the
core names an agent (a core unit test fails when one does); an agent is data.

Each agent the core ships is a **descriptor**: one JSON file, named after the agent, that
says how it runs commands on its own events (below). An index names which shipped agent is
meant when `--agent` is not given. Beside them are the two texts the core writes into a
project: the authoring guide `docattest skill install` copies to where the agent looks for
skills, and the section `docattest agents install` writes into a project's `AGENTS.md`, in
which `{docattest}` stands for the command that runs docattest.

The core embeds all of them when it is built (`core/build.rs`, and `include_str!` in
`core/src/cli/skill.rs` and `core/src/cli/agents.rs`), so a change here is a change to the
core. `protocol/sync.ts` and `adapters/python/sync_protocol.py` also copy the `*.json`
files into each package.

How docattest works with agents (the two plain commands, `AGENTS.md`, the git hook, and
what hooks do) is user documentation: [docs/agents.md](../../docs/agents.md). This page is
for adding an agent.

## What a descriptor is

An agent that runs commands on its own events can be made to run docattest without being
told: `docattest impact --hook` after it edits a file, and `docattest check --hook` before it
stops. Every agent spells this differently: its own settings file, its own event names, its
own JSON on standard input and its own JSON for "add this to your context" and "do not stop
yet, because". A descriptor says, for one agent, where each of those things is. There is no
template language: a place in a JSON object is a list of keys, and an answer is a fixed
object with one string put into it.

Here is `gemini.json`, annotated:

```jsonc
{
  "$schema": "../schema/agent.json#/$defs/Agent",
  "name": "gemini",                                     // what --agent takes
  "docs": "https://geminicli.com/docs/hooks/reference/", // the page this was checked against
  "projectRoot": "\"$GEMINI_PROJECT_DIR\"",            // how a hook command writes the root
  "skill": ".gemini/skills/docattest/SKILL.md",         // where skill install writes
  "settings": {
    "file": ".gemini/settings.json",                    // where hooks install writes
    "hooks": "hooks",                                   // key: event name -> list of entries
    "group": "hooks",                                   // key of an entry's list of handlers
    "handler": { "type": "command" },                   // written into every handler
    "command": "command"                                // key of the handler's shell command
  },
  "hooks": {
    "impact": {
      "event": "AfterTool",
      "matcher": "write_file|replace",                  // the tools that edit files
      "file": ["tool_input", "file_path"],              // the edited file, in the event
      "reply": { "text": ["hookSpecificOutput", "additionalContext"] }
    },
    "check": {
      "event": "AfterAgent",
      "continuing": ["stop_hook_active"],               // true (or > 0): already sent back once
      "reply": { "fixed": { "decision": "deny" }, "text": ["reason"] }
    }
  }
}
```

## Every field

The format's definition is [`../schema/agent.json`](../schema/agent.json); the core reads a
descriptor with the same rules and refuses an unknown key, naming its place.

| field | required | what it says |
|---|---|---|
| `name` | yes | the agent's name, as `--agent` takes it: lower-case letters, digits, `-` and `_`, starting with a letter. A shipped descriptor's file is `<name>.json`. |
| `docs` | no (every shipped one has it) | the page of the agent's own documentation the descriptor was checked against |
| `projectRoot` | no | how a hook command of this agent writes the project root, as shell text (`"$CLAUDE_PROJECT_DIR"`). The core puts it where a launcher wrote `{root}`. Leave it out when the agent runs hooks in the project root; `.` is used then. |
| `skill` | no | where the agent looks for a skill, relative to the project root (and to the home directory with `skill install --global`). Leave it out when the agent has no such place. |
| `settings.file` | yes | the JSON file the agent reads hooks from, relative to the project root |
| `settings.localFile` | no | the untracked project-local settings file, relative to the project root, for `hooks install --local` |
| `settings.hooks` | yes | the top-level key of that file that maps an event name to a list of entries |
| `settings.group` | no | when an entry groups handlers under a filter: the key of the entry's list of handlers. Leave it out when an entry is itself a handler. |
| `settings.handler` | no | fields every handler carries besides its command (a type, a shell, a timeout) |
| `settings.command` | yes | the key of a handler that holds the shell command |
| `settings.defaults` | no | top-level keys the file must have, added with these values where missing (a `version`) |
| `hooks.impact` | no | the event after the agent edits a file. Leave it out when the agent has none whose answer reaches the agent. |
| `hooks.impact.event`, `.matcher` | event: yes | the event's name, and the entry's `matcher` where the event needs one to select the editing tools |
| `hooks.impact.file` | yes | where the edited file's path is in the event the agent writes to standard input |
| `hooks.impact.reply` | yes | the answer that puts text into the agent's context: `fixed` (optional) is the object, and `text` is where the text goes in it |
| `hooks.check` | no | the event before the agent stops |
| `hooks.check.event`, `.matcher` | event: yes | as for `impact` |
| `hooks.check.continuing` | no | where the event says the agent is already continuing because a hook sent it back. When the value there is `true` or a number above 0 the hook stays silent, so a failure the agent cannot fix never loops. |
| `hooks.check.reply` | yes | the answer that keeps the agent working, with the failure as its text |

## Check the agent's hook format in its own documentation

A descriptor is shipped only when the agent's documentation says what its hooks look like.
Write down, from that page (and put its address in `docs`):

1. **The settings file** the agent reads hooks from in a project, and its shape: the key that
   holds hooks, whether an entry is a handler or a group of handlers with a filter, the key
   of the command, and anything else a handler or the file must carry.
2. **The event after a file is edited**: its name, how to select the tools that edit files,
   where the edited file's path is in the JSON on the hook's standard input, and what the
   hook must print for its text to reach the agent. If the documentation gives no answer
   that reaches the agent, or does not say where the path is, leave `impact` out (as
   `cursor.json` does) and let `AGENTS.md` carry the reminder.
3. **The event before the agent stops**: its name, what the hook prints to keep the agent
   working and where the reason goes, and the field that says the agent was already sent
   back once (a flag or a counter).
4. **How a hook command refers to the project root**, if hooks are not run in it: usually an
   environment variable, quoted as the shell needs.
5. **Where skills go**, if the agent reads skills.

When the documentation is silent on a point, do not guess: leave the hook out.

## Test a descriptor

Before you open a pull request, try it on a scratch project with the built core (never on
a real project's settings):

```sh
npm run build:core && npm run build                # in the repository
repo="$PWD"
docattest() { node "$repo/scripts/with-core.ts" node "$repo/adapters/typescript/dist/launcher/main.js" "$@"; }
agent="$repo/protocol/agents/<name>.json"
cd "$(mktemp -d)"   # then write a docattest.config.json, a source file and a page into it
docattest hooks install --agent "$agent"
```

Then read the settings file it wrote, and feed each hook a sample event shaped as the
agent's documentation shows it (these two are Gemini CLI's), checking what it prints:

```sh
echo '{"tool_input":{"file_path":"src/index.ts"}}' | docattest impact --hook --agent "$agent"
echo '{"stop_hook_active":false}' | docattest check --hook --agent "$agent"
```

A shipped descriptor is held by these tests, which must pass and be updated with it:

- **The core's unit tests** (`cargo test` in `core/`): every shipped descriptor reads, is
  named as its file is and binds at least one hook, and no core source file mentions what
  one says.
- **`docs/agents.md`**: the page lists the shipped agents, prints the refusal for an unknown
  name (which names every shipped agent), and installs and runs the hooks of each. Add your
  agent to the table and to the examples there.
- **`tests/parity/impact.ts`**: `hooks install`, `impact --hook` and `check --hook` for
  every shipped agent, and for **acme**, an agent nobody ships, made up to differ from the
  shipped ones wherever a descriptor can (where its settings are and how they nest, its
  event names, where an event holds the file and the "already continuing" sign, and the
  shape of an answer), together with a list of malformed descriptors that must be refused.
  Add cases for the new agent and record them on purpose
  (`--update --source "<why>"`, see [tests/README.md](../../tests/README.md)); the existing
  recordings that name the shipped agents change too, and the diff shows exactly where.

## Submit it

Open a pull request with the descriptor, the updated `docs/agents.md`, the new parity cases
and their recordings, and a line in `CHANGELOG.md`. Say which version of the agent you
tested against and link the documentation you used.

## An agent we do not ship: `--agent ./file.json`

Nobody has to wait for a release. Anything given to `--agent` that is not a bare name is a
descriptor file:

```sh
npx docattest hooks install --agent ./tools/my-agent.json
```

The hooks it installs name the file by a path the agent can find again (relative to the
project root, as the agent writes the root), so keep the file in the project. The made-up
`acme` agent in [docs/agents.md](../../docs/agents.md#a-descriptor-of-your-own) is a
working example.
