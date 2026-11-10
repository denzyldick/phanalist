#!/usr/bin/env python3
"""
Generate JSON from phanalist rule example markdown files.
"""
import json
import os
import re
import sys
from pathlib import Path

EXAMPLES_DIR = Path("/home/denzyl/Projects/phanalist/src/rules/examples")
OUTPUT_DIR = Path("/home/denzyl/Projects/phanalist/website/data/rules")

SEVERITY_MAP = {
    "E0001": "major", "E0014": "major", "E0031": "critical", "E0044": "major",
    "E0045": "major", "E0046": "major", "E0049": "minor", "E0051": "major",
    "E0052": "major", "E0053": "major", "E0054": "major", "E0055": "major",
    "E0056": "major", "E0057": "major", "E0058": "major", "E0059": "major",
    "E0060": "major", "E0061": "major", "E0062": "major",
}

CATEGORY_MAP = {
    "E0001": "configuration", "E0014": "law-of-demeter", "E0031": "verification",
    "E0044": "resource", "E0045": "resource", "E0046": "resource",
    "E0049": "syntax", "E0051": "taint", "E0052": "taint", "E0053": "taint",
    "E0054": "taint", "E0055": "taint", "E0056": "taint", "E0057": "taint",
    "E0058": "taint", "E0059": "taint", "E0060": "taint", "E0061": "taint",
    "E0062": "taint",
}


def extract_heading_text(text, level=1):
    """Extract text from a markdown heading of given level."""
    pattern = rf"^{'#' * level}\s+(.+)$"
    match = re.search(pattern, text, re.MULTILINE)
    return match.group(1).strip() if match else ""


def extract_code_block(text, marker):
    """Extract a fenced code block after a marker line."""
    # Find the marker line
    lines = text.split('\n')
    for i, line in enumerate(lines):
        if marker in line:
            # Look for the next fenced code block
            for j in range(i + 1, len(lines)):
                if lines[j].strip().startswith("```"):
                    # Find the closing fence
                    k = j + 1
                    code_lines = []
                    while k < len(lines) and lines[k].strip() != "```":
                        code_lines.append(lines[k])
                        k += 1
                    return '\n'.join(code_lines).strip()
    return ""


def extract_bullets(text):
    """Extract bullet point lines starting with '- '."""
    return [line[2:].strip() for line in text.split('\n') 
            if line.strip().startswith('- ')]


def parse_markdown(filepath: Path) -> dict:
    """Parse a rule example markdown file and extract structured data."""
    text = filepath.read_text(encoding="utf-8")
    
    # Extract rule code from filename or heading
    code_from_file = filepath.stem  # e.g., "e14" from "e14.md"
    
    # Try to extract code from the main heading: "# E0014: ..."
    code = None
    title_match = re.match(r"^(E\d+)", text.split('\n')[0])
    if title_match:
        code = title_match.group(1)
    elif code_from_file.startswith('e'):
        # Extract number from "e14" -> "E0014"
        num = code_from_file[1:]
        code = f"E{int(num):04d}" if num.isdigit() else f"E{num}"
    
    # Title: try main heading first, then sub-heading
    title = ""
    # Main heading: "# E0014: Law of Demeter violation..."
    main_title = extract_heading_text(text, 1)
    if main_title and main_title.startswith('E'):
        title = main_title
    # Sub-heading: "### E0014: The 'Don't Talk to Strangers' Rule..."
    sub_title = extract_heading_text(text, 3)
    if not title and sub_title and sub_title.startswith('E'):
        title = sub_title
    if not title:
        title = extract_heading_text(text, 1)  # fallback to first heading
    
    # Plain English description - try multiple patterns
    plain_english = ""
    for pattern in [
        r"\*\*In plain English:\*\*\s*\n(.*?)\n\n",
        r"### \*\*In plain English:\*\*\s*\n(.*?)\n\n",
        r"## \*\*In plain English:\*\*\s*\n(.*?)\n\n",
    ]:
        m = re.search(pattern, text, re.DOTALL)
        if m:
            plain_english = m.group(1).strip()
            break
    
    # Why it matters
    why_matters = ""
    for pattern in [
        r"\*\*Why it matters:\*\*\s*\n(.*?)\n\n",
        r"### \*\*Why it matters:\*\*\s*\n(.*?)\n\n",
    ]:
        m = re.search(pattern, text, re.DOTALL)
        if m:
            why_matters = m.group(1).strip()
            break
    
    # Bad example - try both ❌ and Bad markers
    bad_example = ""
    for marker in ["## ❌ The Bad Example", "## Bad", "### ❌ The Bad Example"]:
        bad_example = extract_code_block(text, marker)
        if bad_example:
            break
    
    # Good example - try both ✅ and Good markers
    good_example = ""
    for marker in ["## ✅ The Good Example", "## Good", "### ✅ The Good Example"]:
        good_example = extract_code_block(text, marker)
        if good_example:
            break
    
    # What the rule checks - extract bullet points
    what_it_checks = []
    for marker in ["## What the rule checks", "### What the rule checks"]:
        idx = text.find(marker)
        if idx >= 0:
            # Get text after the marker until next ## or end
            after = text[idx + len(marker):]
            next_heading = re.search(r"\n## |^## ", after, re.MULTILINE)
            if next_heading:
                section_text = after[:next_heading.start()]
            else:
                section_text = after
            bullets = extract_bullets(section_text)
            if bullets:
                what_it_checks = bullets
            break
    
    # If no bullets found, try to extract from the "How the rule works" section
    if not what_it_checks:
        hw_match = re.search(r"## How the rule works\s*\n(.*?)(?=\n##|\Z)", text, re.DOTALL)
        if hw_match:
            what_it_checks = extract_bullets(hw_match.group(1))
    
    # If still empty, derive from rule code or title
    if not what_it_checks and code:
        what_it_checks = [f"Checks for {code} violations"]
    
    # Default severity and category if not in map
    severity = SEVERITY_MAP.get(code, "info")
    category = CATEGORY_MAP.get(code, "general")
    
    return {
        "rule_code": code or code_from_file,
        "title": title or code_from_file,
        "plain_english": plain_english,
        "why_matters": why_matters,
        "bad_example": bad_example,
        "good_example": good_example,
        "severity": severity,
        "category": category,
        "what_it_checks": what_it_checks,
    }


def main():
    """Main entry point: parse all example markdown files and output JSON."""
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    
    results = []
    # Walk the examples directory, find all e{N}/e{N}.md files
    for entry in sorted(EXAMPLES_DIR.iterdir()):
        if entry.is_dir():
            md_file = entry / f"{entry.name}.md"
            if md_file.is_file():
                data = parse_markdown(md_file)
                results.append(data)
                
                # Write individual JSON file
                output_file = OUTPUT_DIR / f"{data['rule_code']}.json"
                output_file.write_text(json.dumps(data, indent=2, ensure_ascii=False))
                print(f" Generated: {data['rule_code']}: {data['title'][:50]}...")
    
    # Also write a combined index
    index_file = OUTPUT_DIR / "index.json"
    index_file.write_text(json.dumps({"rules": results}, indent=2, ensure_ascii=False))
    print(f"\nGenerated index: {index_file} ({len(results)} rules)")


if __name__ == "__main__":
    main()
