"""Renders a fenced `yaml table` block as a table on the docs site.

The block holds a list of rows, each a mapping from column to cell, and the
first row's keys name the columns. On GitHub the block stays a YAML code
block, which wraps at 80 columns where a Markdown table can't:

    ```yaml table
    - jj concept: Bookmark
      Closest git equivalent: Branch
    ```
"""

import markdown
import yaml


def validate(language, inputs, options, attrs, md):
    """Claims a yaml fence only when its header says table, so the rest
    stay highlighted code."""
    return "table" in inputs


def render(source, language, class_name, options, md, **kwargs):
    rows = yaml.safe_load(source)
    columns = list(rows[0])
    lines = [_row(columns), _row(["---"] * len(columns))]
    lines += [_row([row[column] for column in columns]) for row in rows]
    return markdown.markdown("\n".join(lines), extensions=["tables"])


def _row(cells):
    return "| " + " | ".join(str(cell).strip().replace("|", "\\|") for cell in cells) + " |"
