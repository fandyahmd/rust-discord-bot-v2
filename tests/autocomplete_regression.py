"""Run: python3 tests/autocomplete_regression.py

Guard against zero Serenity IDs in the autocomplete handler: constructing one
panics even inside an eagerly evaluated Option::unwrap_or fallback.
"""
from pathlib import Path
import re

source = (Path(__file__).resolve().parents[1] / "src/autocomplete.rs").read_text()
assert not re.search(r"GuildId::new\(\s*0\s*\)", source), "autocomplete constructs an invalid zero guild ID"
print("autocomplete zero-ID regression guard passed")
