"""Errors the shop raises."""

ERROR_CODES = {
    "out_of_stock": "The item is not available",
    "bad_discount": "The discount is outside the allowed range",
    "unknown_region": "No carrier serves the region",
}


class ShopError(Exception):
    """An error with one of the codes in ``ERROR_CODES``."""

    def __init__(self, code: str):
        super().__init__(ERROR_CODES[code])
        self.code = code
