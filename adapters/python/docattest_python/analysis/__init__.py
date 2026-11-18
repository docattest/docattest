"""Reading Python modules without importing them: what a module binds at its top level,
what it exports, where each export is declared, the values of its constants, which names a
piece of code uses, what a configured surface resolves to and what a fact may read of it,
and which exports a change affects. ``declared_tests`` reads what a test file declares.

Everything is syntax (``ast``); no project code runs. ``Project`` reads files through a
function, so the same code reads the working tree and the project as it was at a base
revision (``impact``).
"""
