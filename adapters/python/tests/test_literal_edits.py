"""The edits ``mutants`` offers for a declaration's literals: what each kind of literal
becomes, in which order, and that every edit leaves code that compiles.

    python3 -m unittest discover -s adapters/python/tests -v
"""

import ast
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))

from docattest_python.analysis.literal_edits import literal_edits  # noqa: E402
from docattest_python.analysis.modules import Declaration, Module  # noqa: E402


def mutated(source, name):
    """Each edit of ``name``'s literals: its description, and the declaration it leaves
    (the module's text from where the name is assigned, to the end of that statement)."""
    out = []
    for edit in literal_edits(source, _declaration(source, name)):
        encoded = source.encode("utf-8")
        changed = (encoded[: edit.start] + edit.replacement.encode("utf-8") + encoded[edit.end :]).decode()
        compile(changed, "m.py", "exec")
        out.append((edit.description, _statement(changed, name)))
    return out


def _statement(text, name):
    """The statement that assigns ``name``, as the module now writes it."""
    node = _declaration(text, name).nodes[0]
    assert isinstance(node, ast.stmt)
    lines = text.splitlines()[node.lineno - 1 : node.end_lineno]
    return "\n".join(lines)


def _declaration(text, name):
    binding = Module("m", "m.py", text, False).bindings[name]
    assert isinstance(binding, Declaration)
    return binding


class Numbers(unittest.TestCase):
    def test_an_int_gains_one_written_in_decimal(self):
        self.assertEqual(mutated("PORT = 8080\n", "PORT"), [("8080 → 8081", "PORT = 8081")])
        self.assertEqual(mutated("MASK = 0xFF\n", "MASK"), [("0xFF → 256", "MASK = 256")])

    def test_a_float_gains_one_written_as_python_writes_a_float(self):
        self.assertEqual(mutated("RATE = 1.5\n", "RATE"), [("1.5 → 2.5", "RATE = 2.5")])
        self.assertEqual(mutated("BIG = 1e3\n", "BIG"), [("1e3 → 1001.0", "BIG = 1001.0")])
        self.assertEqual(mutated("IMAGINARY = 2j\n", "IMAGINARY"), [("2j → 3j", "IMAGINARY = 3j")])

    def test_a_float_that_is_no_literal_or_cannot_change_is_left_alone(self):
        self.assertEqual(mutated("INFINITE = 1e999\nHUGE = 1e300\n", "INFINITE"), [])
        self.assertEqual(mutated("HUGE = 1e300\n", "HUGE"), [])

    def test_a_bool_is_flipped(self):
        self.assertEqual(mutated("ON = True\n", "ON"), [("True → False", "ON = False")])

    def test_an_annotation_is_not_the_value(self):
        self.assertEqual(mutated("OFF: 'bool' = False\n", "OFF"), [("False → True", "OFF: 'bool' = True")])

    def test_none_and_ellipsis_are_left_alone(self):
        self.assertEqual(mutated("NOTHING = None\n", "NOTHING"), [])
        self.assertEqual(mutated("ANY = ...\n", "ANY"), [])


class Strings(unittest.TestCase):
    def test_a_string_gains_x_before_its_closing_quote(self):
        self.assertEqual(mutated("NAME = 'shop'\n", "NAME"), [("'shop' → 'shopx'", "NAME = 'shopx'")])
        self.assertEqual(mutated('NAME = "shop"\n', "NAME"), [('"shop" → "shopx"', 'NAME = "shopx"')])

    def test_escapes_and_prefixes_are_kept(self):
        self.assertEqual(mutated('QUOTE = "a\\"b"\n', "QUOTE"), [('"a\\"b" → "a\\"bx"', 'QUOTE = "a\\"bx"')])
        self.assertEqual(
            mutated("PATTERN = r'\\d+'\n", "PATTERN"), [("r'\\d+' → r'\\d+x'", "PATTERN = r'\\d+x'")]
        )
        self.assertEqual(mutated('WORD = u"w"\n', "WORD"), [('u"w" → u"wx"', 'WORD = u"wx"')])

    def test_bytes_gain_x_too(self):
        self.assertEqual(mutated("MAGIC = b'PK'\n", "MAGIC"), [("b'PK' → b'PKx'", "MAGIC = b'PKx'")])
        self.assertEqual(mutated("RAW = Rb'\\0'\n", "RAW"), [("Rb'\\0' → Rb'\\0x'", "RAW = Rb'\\0x'")])

    def test_a_triple_quoted_string_keeps_its_quotes(self):
        source = 'TEXT = """one\ntwo"""\n'
        self.assertEqual(
            mutated(source, "TEXT"), [('"""one\ntwo""" → """one\ntwox"""', 'TEXT = """one\ntwox"""')]
        )

    def test_an_implicit_concatenation_is_one_literal_per_part(self):
        source = "GREETING = (\"hello, \"\n            'world')\n"
        self.assertEqual(
            mutated(source, "GREETING"),
            [
                ('"hello, " → "hello, x"', "GREETING = (\"hello, x\"\n            'world')"),
                ("'world' → 'worldx'", "GREETING = (\"hello, \"\n            'worldx')"),
            ],
        )

    def test_an_f_string_is_no_literal(self):
        self.assertEqual(mutated('PORT = 1\nURL = f"http://h:{PORT}" "/x"\n', "URL"), [])

    def test_offsets_are_in_utf8_bytes(self):
        source = 'LABEL = ("é€", "ok")\n'
        self.assertEqual(
            mutated(source, "LABEL"),
            [
                ('"ok" removed', 'LABEL = ("é€",)'),
                ('"é€" → "é€x"', 'LABEL = ("é€x", "ok")'),
                ('"ok" → "okx"', 'LABEL = ("é€", "okx")'),
            ],
        )


class Displays(unittest.TestCase):
    def test_a_list_offers_its_last_item_removed_before_its_items(self):
        self.assertEqual(
            mutated('KEYS = ["A", "B"]\n', "KEYS"),
            [
                ('"B" removed', 'KEYS = ["A",]'),
                ('"A" → "Ax"', 'KEYS = ["Ax", "B"]'),
                ('"B" → "Bx"', 'KEYS = ["A", "Bx"]'),
            ],
        )

    def test_a_pair_stays_a_tuple(self):
        self.assertEqual(mutated("PAIR = (1, (2))\n", "PAIR")[0], ("2 removed", "PAIR = (1,)"))
        self.assertEqual(mutated("PAIR = 1, 2\n", "PAIR")[0], ("2 removed", "PAIR = 1,"))

    def test_a_trailing_comma_goes_with_the_last_item(self):
        source = "LEVELS = [\n    1,\n    2,\n]\n"
        self.assertEqual(mutated(source, "LEVELS")[0], ("2 removed", "LEVELS = [\n    1,\n]"))

    def test_a_single_item_leaves_an_empty_display_of_the_same_kind(self):
        self.assertEqual(mutated("ONE = [1]\n", "ONE")[0], ("1 removed", "ONE = []"))
        self.assertEqual(mutated("ONE = (1,)\n", "ONE")[0], ("1 removed", "ONE = ()"))
        self.assertEqual(mutated("ONE = 1,\n", "ONE")[0], ("1 removed", "ONE = ()"))
        self.assertEqual(mutated("ONE = {'x'}\n", "ONE")[0], ("'x' removed", "ONE = set()"))

    def test_an_empty_display_offers_nothing(self):
        self.assertEqual(mutated("NONE = []\n", "NONE"), [])

    def test_nested_literals_come_in_source_order_and_dict_keys_are_literals(self):
        source = 'LIMITS = {"small": [1, (2, 3)], 4: None}\n'
        self.assertEqual(
            [description for description, _ in mutated(source, "LIMITS")],
            [
                '"small" → "smallx"',
                "(2, 3) removed",
                "1 → 2",
                "3 removed",
                "2 → 3",
                "3 → 4",
                "4 → 5",
            ],
        )


class Values(unittest.TestCase):
    def test_a_lambda_is_not_followed_but_a_call_is(self):
        self.assertEqual(mutated("LATER = lambda: 5\n", "LATER"), [])
        self.assertEqual(
            mutated("OPTIONS = dict(retries=3)\n", "OPTIONS"), [("3 → 4", "OPTIONS = dict(retries=4)")]
        )

    def test_an_unpacking_gives_each_name_its_own_item(self):
        source = "WIDTH, HEIGHT = 640, 480\n"
        self.assertEqual(mutated(source, "HEIGHT"), [("480 → 481", "WIDTH, HEIGHT = 640, 481")])

    def test_every_assignment_of_the_name_is_read(self):
        source = "try:\n    LEVEL = 1\nexcept ImportError:\n    LEVEL = 2\n"
        self.assertEqual([d for d, _ in mutated(source, "LEVEL")], ["1 → 2", "2 → 3"])

    def test_a_non_literal_constant_mutates_as_an_expression(self):
        self.assertEqual(
            mutated("TIMEOUT = SECOND\n", "TIMEOUT"),
            [("SECOND → 2 * SECOND", "TIMEOUT = 2 * SECOND")],
        )
        self.assertEqual(
            mutated("LABEL: str = PREFIX\n", "LABEL"),
            [('PREFIX → PREFIX + "x"', 'LABEL: str = PREFIX + "x"')],
        )
        self.assertEqual(mutated("FLAG: bool = ON\n", "FLAG"), [("ON → not ON", "FLAG: bool = not ON")])


if __name__ == "__main__":
    unittest.main()
