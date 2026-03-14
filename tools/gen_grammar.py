#!/usr/bin/env python3
"""
gen_grammar.py — Generate vscode-wilios/syntaxes/wilios.tmLanguage.json
from doc/grammar.ebnf.

Keyword lists are read dynamically from the RESERVED KEYWORDS comment block
in the EBNF file. Terminal regex patterns (pitch, duration, float, integer,
operators, etc.) are derived from the EBNF terminal rules and hardcoded here
since they are structural constants of the language spec.

Usage:
    python3 tools/gen_grammar.py
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).parent.parent
EBNF_PATH = ROOT / "doc" / "grammar.ebnf"
OUT_PATH = ROOT / "vscode-wilios" / "syntaxes" / "wilios.tmLanguage.json"

# ---------------------------------------------------------------------------
# Scope mapping: EBNF keyword-group label → VS Code TM scope
# "Control" is split further below (let, func get their own scopes).
# ---------------------------------------------------------------------------
LABEL_SCOPE = {
    "Control":  "keyword.control.wilios",
    "Musical":  "keyword.other.musical.wilios",
    "Synth":    "keyword.other.synth.wilios",
    "FM block": "keyword.other.fm.wilios",
    "Waveforms":"support.constant.waveform.wilios",
    "Import":   "keyword.control.import.wilios",
    "Literals": "constant.language.boolean.wilios",
}

# These Control-group keywords get their own dedicated scopes/rules.
SPECIAL_CONTROL = {
    "let":    "keyword.other.declaration.wilios",
    "func":   "keyword.other.function.wilios",
}


def parse_keyword_groups(ebnf: str) -> dict[str, list[str]]:
    """
    Extract keyword groups from the RESERVED KEYWORDS comment block.

    The block is a single (* ... *) comment. Inside it, keyword lines look like:
        Label:   kw1  kw2  kw3
    where "Label" may contain spaces (e.g. "FM block").

    Returns {label: [kw, ...]} preserving order.
    """
    # Find the RESERVED KEYWORDS section (one big block comment).
    # The section ends with a line of '===...===  *)'
    section_match = re.search(
        r'RESERVED KEYWORDS.*?(?=\s*={10,}\s*\*\))',
        ebnf,
        re.DOTALL,
    )
    if not section_match:
        sys.exit("ERROR: Could not find RESERVED KEYWORDS section in grammar.ebnf")

    section = section_match.group(0)
    groups: dict[str, list[str]] = {}

    # Each keyword line: optional whitespace, Label: kw1  kw2  ...
    for m in re.finditer(r'^\s+([\w][\w ]*?):\s+([a-z_][\w\s_]*?)$', section, re.MULTILINE):
        label = m.group(1).strip()
        keywords = m.group(2).split()
        if label in LABEL_SCOPE:
            groups[label] = keywords

    if not groups:
        sys.exit("ERROR: No keyword groups found in RESERVED KEYWORDS section")

    return groups


def kw_pattern(keywords: list[str]) -> str:
    """Build a word-boundary alternation pattern for a list of keywords."""
    alts = "|".join(re.escape(k) for k in keywords)
    return rf"\b({alts})\b"


def build_grammar(groups: dict[str, list[str]]) -> dict:
    """Construct the full tmLanguage grammar dict."""

    # Split "Control" into special (let, func) and the rest
    control_all = groups.get("Control", [])
    control_special = [k for k in control_all if k in SPECIAL_CONTROL]
    control_rest = [k for k in control_all if k not in SPECIAL_CONTROL]

    repository = {}

    # ------------------------------------------------------------------
    # 1. Line comments  (from: comment = "//" , ... , newline)
    # ------------------------------------------------------------------
    repository["comments"] = {
        "name": "comment.line.double-slash.wilios",
        "match": r"//.*$",
    }

    # ------------------------------------------------------------------
    # 2. String literals  (from: string_lit = '"' , {string_char} , '"')
    #    with escape sequences: \n \t \\ \"
    # ------------------------------------------------------------------
    repository["strings"] = {
        "name": "string.quoted.double.wilios",
        "begin": r'"',
        "end": r'"',
        "patterns": [
            {
                "name": "constant.character.escape.wilios",
                "match": r'\\[nrt\\"]',
            },
            {
                "name": "invalid.illegal.escape.wilios",
                "match": r'\\.',
            },
        ],
    }

    # ------------------------------------------------------------------
    # 3. Keyword groups (from RESERVED KEYWORDS block)
    # ------------------------------------------------------------------

    # 3a. Special control keywords (let, func) — each gets its own rule
    for kw in control_special:
        repository[f"keyword-{kw}"] = {
            "name": SPECIAL_CONTROL[kw],
            "match": rf"\b({re.escape(kw)})\b",
        }

    # 3b. Remaining control keywords
    if control_rest:
        repository["keywords-control"] = {
            "name": LABEL_SCOPE["Control"],
            "match": kw_pattern(control_rest),
        }

    # 3c. All other groups from EBNF
    for label, keywords in groups.items():
        if label == "Control":
            continue  # handled above
        key = "keywords-" + label.lower().replace(" ", "-")
        repository[key] = {
            "name": LABEL_SCOPE[label],
            "match": kw_pattern(keywords),
        }

    # ------------------------------------------------------------------
    # 4. Chord expression  (from: chord_stmt = "<" , expr , ... , ">")
    #    Uses begin/end so that pitches inside are matched but operators
    #    outside (like < in comparisons) are handled by the operator rule.
    # ------------------------------------------------------------------
    repository["chord-expression"] = {
        "name": "meta.chord.wilios",
        "begin": r"<(?=[A-G])",
        "end": r">",
        "beginCaptures": {"0": {"name": "punctuation.definition.chord.begin.wilios"}},
        "endCaptures": {"0": {"name": "punctuation.definition.chord.end.wilios"}},
        "patterns": [
            {"include": "#pitches"},
            {"name": "punctuation.separator.comma.wilios", "match": r","},
        ],
    }

    # ------------------------------------------------------------------
    # 5. Pitch tokens  (from: pitch = letter , [accidental] , octave_digit)
    #    letter = A|B|C|D|E|F|G
    #    accidental = "#" | "b"
    #    octave_digit = 0-9
    # ------------------------------------------------------------------
    repository["pitches"] = {
        "name": "constant.language.pitch.wilios",
        "match": r"\b([A-G][#b]?[0-9])\b",
    }

    # ------------------------------------------------------------------
    # 6. Duration literals  (from: duration_lit = integer "/" integer ["."])
    #    Must come before floats and integers to avoid partial matches.
    # ------------------------------------------------------------------
    repository["durations"] = {
        "name": "constant.numeric.duration.wilios",
        "match": r"\b([0-9]+\/[0-9]+\.?)\b",
    }

    # ------------------------------------------------------------------
    # 7. Float literals  (from: float = digit+ "." digit+)
    # ------------------------------------------------------------------
    repository["floats"] = {
        "name": "constant.numeric.float.wilios",
        "match": r"\b([0-9]+\.[0-9]+)\b",
    }

    # ------------------------------------------------------------------
    # 8. Integer literals  (from: integer = digit+)
    # ------------------------------------------------------------------
    repository["integers"] = {
        "name": "constant.numeric.integer.wilios",
        "match": r"\b([0-9]+)\b",
    }

    # ------------------------------------------------------------------
    # 9. Operators  (derived from expression precedence table in EBNF)
    #    Order: multi-char tokens before single-char to avoid ambiguity.
    # ------------------------------------------------------------------
    repository["operators"] = {
        "patterns": [
            # FM routing arrow (from: routing_edge = integer , "->" , integer)
            {
                "name": "keyword.operator.arrow.wilios",
                "match": r"->",
            },
            # Comparison (from: eq_expr, cmp_expr)
            {
                "name": "keyword.operator.comparison.wilios",
                "match": r"(==|!=|<=|>=)",
            },
            # Logical (from: or_expr, and_expr)
            {
                "name": "keyword.operator.logical.wilios",
                "match": r"(&&|\|\|)",
            },
            # Assignment =  (not ==, not inside <= >= != ==)
            {
                "name": "keyword.operator.assignment.wilios",
                "match": r"(?<![=!<>])=(?!=)",
            },
            # Arithmetic + - * % (/ handled below to avoid duration clash)
            {
                "name": "keyword.operator.arithmetic.wilios",
                "match": r"[+\-*%]",
            },
            # Division / only when NOT between digit-adjacent chars (would be duration)
            {
                "name": "keyword.operator.arithmetic.wilios",
                "match": r"(?<![0-9])\/(?![0-9])",
            },
        ]
    }

    # ------------------------------------------------------------------
    # 10. Function calls — identifier immediately followed by "("
    #     (from: call_expr, call_stmt)
    # ------------------------------------------------------------------
    repository["function-calls"] = {
        "match": r"\b([a-z_][a-z_0-9]*)\b(?=\s*\()",
        "captures": {"1": {"name": "entity.name.function.wilios"}},
    }

    # ------------------------------------------------------------------
    # 11. Identifiers  (from: ident = lowercase , {lowercase | "_" | digit})
    # ------------------------------------------------------------------
    repository["identifiers"] = {
        "name": "variable.other.wilios",
        "match": r"\b[a-z_][a-z_0-9]*\b",
    }

    # ------------------------------------------------------------------
    # 12. Punctuation
    # ------------------------------------------------------------------
    repository["punctuation"] = {
        "patterns": [
            {"name": "punctuation.section.block.begin.wilios",   "match": r"\{"},
            {"name": "punctuation.section.block.end.wilios",     "match": r"\}"},
            {"name": "punctuation.section.parens.begin.wilios",  "match": r"\("},
            {"name": "punctuation.section.parens.end.wilios",    "match": r"\)"},
            {"name": "punctuation.section.brackets.begin.wilios","match": r"\["},
            {"name": "punctuation.section.brackets.end.wilios",  "match": r"\]"},
            {"name": "punctuation.separator.comma.wilios",       "match": r","},
        ]
    }

    # ------------------------------------------------------------------
    # Top-level patterns list — order determines priority (first match wins)
    # ------------------------------------------------------------------
    top_patterns = [
        {"include": "#comments"},
        {"include": "#strings"},
    ]

    # Special control keywords before the group rule
    for kw in control_special:
        top_patterns.append({"include": f"#keyword-{kw}"})

    top_patterns += [
        {"include": "#keywords-control"},
        {"include": "#keywords-musical"},
        {"include": "#keywords-synth"},
        {"include": "#keywords-fm-block"},
        {"include": "#keywords-waveforms"},
        {"include": "#keywords-import"},
        {"include": "#keywords-literals"},
        {"include": "#chord-expression"},
        {"include": "#pitches"},
        {"include": "#durations"},
        {"include": "#floats"},
        {"include": "#integers"},
        {"include": "#operators"},
        {"include": "#function-calls"},
        {"include": "#identifiers"},
        {"include": "#punctuation"},
    ]

    return {
        "name": "wilios",
        "scopeName": "source.wilios",
        "fileTypes": ["wilios"],
        "patterns": top_patterns,
        "repository": repository,
    }


def main() -> None:
    ebnf = EBNF_PATH.read_text(encoding="utf-8")
    groups = parse_keyword_groups(ebnf)

    print("Keyword groups extracted from grammar.ebnf:")
    for label, kws in groups.items():
        print(f"  {label}: {kws}")

    grammar = build_grammar(groups)

    OUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUT_PATH.write_text(
        json.dumps(grammar, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    print(f"\nWrote {OUT_PATH.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
