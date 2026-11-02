"""Packs Phosphor's bold and fill SVGs into assets/phosphor.tsv (name<TAB>inner SVG).

Usage: python tools/pack_icons.py <unpacked @phosphor-icons/core package dir>
"""
import re
import sys
from pathlib import Path

source, out = Path(sys.argv[1]) / "assets", Path(__file__).parent.parent / "assets" / "phosphor.tsv"
inner = re.compile(r"<svg[^>]*>(.*)</svg>", re.S)
lines = []
for style in ("bold", "fill"):
    for svg in sorted((source / style).glob("*.svg")):
        body = inner.search(svg.read_text(encoding="utf-8")).group(1)
        assert "\t" not in body and "\n" not in body, svg
        lines.append(f"{svg.stem}\t{body}")
assert len(lines) > 2000
out.parent.mkdir(exist_ok=True)
out.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
print(len(lines), "icons,", out.stat().st_size // 1024, "KB")
