## Documentation that is checked (docattest)

The documentation of this project carries its own proof: examples that run, values read
from the code, tables whose rows run. `{docattest} check` runs every proof and fails at the
page and line that is no longer true. Whatever agent you are, do these two things, from the
directory this file is in:

- **After you edit source code**, run `{docattest} impact --file <path>` with the path of
  the file you edited. It prints the documented exports your change affects and every page
  and line that describes them, and prints nothing when there are none. Read the lines it
  lists and bring them up to date with the code.
- **Before you finish**, run `{docattest} check`. If it fails, make the page true again or
  fix the code. Never delete or weaken a claim to make it pass.

`{docattest} where <symbol>` lists the pages that describe an export.

This section is written by `{docattest} agents install`, which replaces everything between
the two markers. Write your own notes outside them.
