"""Reading Python modules without importing them: what a module binds at its top level,
what it exports, where each export is declared, which names a piece of code uses, what a
configured surface resolves to, and which exports a change affects.

Everything is syntax (``ast``); no project code runs. ``Project`` reads files through a
function, so the same code reads the working tree and the project as it was at a base
revision (``impact``).
"""
