#!/usr/bin/env python3
"""Bundle the site into a single self-contained HTML (file:// safe).
Inlines css/*.css and all <script src> files in original order.
Usage: python3 bundle.py [outfile]
"""
import re, sys, os

ROOT = os.path.dirname(os.path.abspath(__file__))
html = open(os.path.join(ROOT, "index.html"), encoding="utf-8").read()

def css_repl(m):
    css = open(os.path.join(ROOT, m.group(1)), encoding="utf-8").read()
    return "<style>\n" + css + "\n</style>"
html = re.sub(r'<link rel="stylesheet" href="(css/[^"]+)">', css_repl, html)

def repl(m):
    src = m.group(1)
    code = open(os.path.join(ROOT, src), encoding="utf-8").read()
    return "<script>\n" + code + "\n</script>"
html = re.sub(r'<script src="([^"]+)"></script>', repl, html)

out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "dist-single.html")
open(out, "w", encoding="utf-8").write(html)
print(out, len(html), "bytes")
