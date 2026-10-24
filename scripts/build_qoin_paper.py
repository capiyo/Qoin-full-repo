#!/usr/bin/env python3
"""Render the Qoin research paper Markdown into a readable PDF using ReportLab."""
from __future__ import annotations
import html
import os
import re
from pathlib import Path

from reportlab.lib import colors
from reportlab.lib.enums import TA_CENTER, TA_LEFT
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle
from reportlab.lib.units import mm
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.platypus import (
    BaseDocTemplate, Frame, PageTemplate, Paragraph, Spacer, PageBreak,
    Table, TableStyle, Preformatted, KeepTogether,
)

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "docs" / "QOIN_ALGORITHM_OBSERVATIONS_MATH_PHILOSOPHY.md"
OUTPUT = ROOT / "docs" / "QOIN_ALGORITHM_OBSERVATIONS_MATH_PHILOSOPHY.pdf"

def register_fonts():
    candidates = [
        ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
         "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"),
        ("/Library/Fonts/Arial.ttf", "/Library/Fonts/Arial Bold.ttf", "/Library/Fonts/Courier New.ttf"),
    ]
    for regular, bold, mono in candidates:
        if all(Path(p).exists() for p in (regular, bold, mono)):
            pdfmetrics.registerFont(TTFont("QoinSans", regular))
            pdfmetrics.registerFont(TTFont("QoinSans-Bold", bold))
            pdfmetrics.registerFont(TTFont("QoinMono", mono))
            pdfmetrics.registerFontFamily("QoinSans", normal="QoinSans", bold="QoinSans-Bold", italic="QoinSans", boldItalic="QoinSans-Bold")
            return "QoinSans", "QoinSans-Bold", "QoinMono"
    return "Helvetica", "Helvetica-Bold", "Courier"

REG, BOLD, MONO = register_fonts()
styles = getSampleStyleSheet()
styles.add(ParagraphStyle(name="QTitle", fontName=BOLD, fontSize=24, leading=30, alignment=TA_CENTER, textColor=colors.HexColor("#163A5F"), spaceAfter=16))
styles.add(ParagraphStyle(name="QSubtitle", fontName=REG, fontSize=11, leading=16, alignment=TA_CENTER, textColor=colors.HexColor("#43566B"), spaceAfter=8))
styles.add(ParagraphStyle(name="QH1", fontName=BOLD, fontSize=16, leading=21, textColor=colors.HexColor("#163A5F"), spaceBefore=13, spaceAfter=7, keepWithNext=True))
styles.add(ParagraphStyle(name="QH2", fontName=BOLD, fontSize=12, leading=16, textColor=colors.HexColor("#176B62"), spaceBefore=10, spaceAfter=5, keepWithNext=True))
styles.add(ParagraphStyle(name="QH3", fontName=BOLD, fontSize=10, leading=13, textColor=colors.HexColor("#25364A"), spaceBefore=8, spaceAfter=4, keepWithNext=True))
styles.add(ParagraphStyle(name="QBody", fontName=REG, fontSize=8.7, leading=12.5, spaceAfter=6, textColor=colors.HexColor("#202B36")))
styles.add(ParagraphStyle(name="QBullet", fontName=REG, fontSize=8.5, leading=11.5, leftIndent=13, firstLineIndent=-8, spaceAfter=3))
styles.add(ParagraphStyle(name="QCode", fontName=MONO, fontSize=7.1, leading=9.2, leftIndent=8, rightIndent=8, borderColor=colors.HexColor("#D7E0E8"), borderWidth=.5, borderPadding=6, backColor=colors.HexColor("#F4F7FA"), spaceBefore=4, spaceAfter=7))
styles.add(ParagraphStyle(name="QCell", fontName=REG, fontSize=6.7, leading=8.6, spaceAfter=0))
styles.add(ParagraphStyle(name="QCellHead", fontName=BOLD, fontSize=6.8, leading=8.8, textColor=colors.white, spaceAfter=0))
styles.add(ParagraphStyle(name="QCallout", fontName=BOLD, fontSize=9, leading=13, borderColor=colors.HexColor("#C9D8E5"), borderWidth=.6, borderPadding=8, backColor=colors.HexColor("#F2F6FA"), spaceBefore=5, spaceAfter=8))

def inline_markup(text: str) -> str:
    text = html.escape(text, quote=False)
    # Inline math is displayed in monospace to preserve notation faithfully.
    text = re.sub(r"\\\((.+?)\\\)", lambda m: '<font name="%s">%s</font>' % (MONO, m.group(1)), text)
    text = re.sub(r"\\\[(.+?)\\\]", lambda m: '<font name="%s">%s</font>' % (MONO, m.group(1)), text)
    text = re.sub(r"\*\*(.+?)\*\*", r"<b>\1</b>", text)
    text = re.sub(r"(?<!\*)\*([^*]+)\*(?!\*)", r"<i>\1</i>", text)
    text = re.sub(r"`([^`]+)`", lambda m: '<font name="%s">%s</font>' % (MONO, m.group(1)), text)
    return text

def split_table_row(line: str):
    return [cell.strip() for cell in line.strip().strip("|").split("|")]

class PaperDoc(BaseDocTemplate):
    def __init__(self, filename):
        super().__init__(filename, pagesize=A4, leftMargin=18*mm, rightMargin=18*mm, topMargin=19*mm, bottomMargin=18*mm, title="Qoin: A Dynamic Market-Force Vector Framework", author="Qoin Research")
        frame = Frame(self.leftMargin, self.bottomMargin, self.width, self.height, id="normal", leftPadding=0, rightPadding=0, topPadding=0, bottomPadding=0)
        self.addPageTemplates([PageTemplate(id="paper", frames=frame, onPage=self.decorate)])
    def decorate(self, canvas, doc):
        canvas.saveState()
        w, h = A4
        if doc.page > 1:
            canvas.setStrokeColor(colors.HexColor("#D7E0E8"))
            canvas.line(18*mm, h-13*mm, w-18*mm, h-13*mm)
            canvas.setFont(REG, 7)
            canvas.setFillColor(colors.HexColor("#657587"))
            canvas.drawString(18*mm, h-10.5*mm, "QOIN | OBSERVATIONS, MATHEMATICS & PHILOSOPHY")
            canvas.drawRightString(w-18*mm, 10*mm, f"Page {doc.page}")
        canvas.restoreState()

def main():
    source = SOURCE.read_text(encoding="utf-8")
    lines = source.splitlines()
    story = []
    i = 0
    first_heading = True
    while i < len(lines):
        line = lines[i].rstrip()
        if not line.strip():
            i += 1
            continue
        if line.strip() == "---":
            story.append(Spacer(1, 4))
            i += 1
            continue
        if line.startswith("```"):
            i += 1
            code = []
            while i < len(lines) and not lines[i].startswith("```"):
                code.append(lines[i])
                i += 1
            i += 1
            story.append(Preformatted("\n".join(code), styles["QCode"], maxLineLength=105))
            continue
        if line.startswith("|") and i + 1 < len(lines) and lines[i+1].lstrip().startswith("|"):
            raw_rows = []
            while i < len(lines) and lines[i].strip().startswith("|"):
                row = split_table_row(lines[i])
                if not all(re.fullmatch(r":?-{3,}:?", c.replace(" ", "")) for c in row):
                    raw_rows.append(row)
                i += 1
            if raw_rows:
                ncols = max(map(len, raw_rows))
                width = (A4[0] - 36*mm) / ncols
                data = []
                for ri, row in enumerate(raw_rows):
                    row = row + [""] * (ncols-len(row))
                    style = styles["QCellHead"] if ri == 0 else styles["QCell"]
                    data.append([Paragraph(inline_markup(c), style) for c in row])
                table = Table(data, colWidths=[width]*ncols, repeatRows=1, hAlign="LEFT")
                table.setStyle(TableStyle([
                    ("BACKGROUND",(0,0),(-1,0),colors.HexColor("#244B6B")),
                    ("GRID",(0,0),(-1,-1),.35,colors.HexColor("#C9D4DE")),
                    ("VALIGN",(0,0),(-1,-1),"TOP"),
                    ("LEFTPADDING",(0,0),(-1,-1),4),
                    ("RIGHTPADDING",(0,0),(-1,-1),4),
                    ("TOPPADDING",(0,0),(-1,-1),4),
                    ("BOTTOMPADDING",(0,0),(-1,-1),4),
                    ("ROWBACKGROUNDS",(0,1),(-1,-1),[colors.white,colors.HexColor("#F4F7FA")]),
                ]))
                story.extend([Spacer(1,4), table, Spacer(1,7)])
            continue
        if line.startswith("# "):
            title = line[2:].strip()
            if first_heading:
                story.extend([Spacer(1, 45*mm), Paragraph(inline_markup(title), styles["QTitle"])])
                first_heading = False
            else:
                story.append(Paragraph(inline_markup(title), styles["QH1"]))
            i += 1
            continue
        if line.startswith("## "):
            title = line[3:].strip()
            if title == "Observational foundations, mathematical specification, and philosophy of decision-making":
                story.append(Paragraph(inline_markup(title), styles["QSubtitle"]))
            else:
                story.append(Paragraph(inline_markup(title), styles["QH1"]))
            i += 1
            continue
        if line.startswith("### "):
            story.append(Paragraph(inline_markup(line[4:].strip()), styles["QH2"]))
            i += 1
            continue
        if line.startswith("#### "):
            story.append(Paragraph(inline_markup(line[5:].strip()), styles["QH3"]))
            i += 1
            continue
        if line.startswith("> "):
            story.append(Paragraph(inline_markup(line[2:]), styles["QCallout"]))
            i += 1
            continue
        if re.match(r"^\d+\.\s+", line):
            story.append(Paragraph("• " + inline_markup(re.sub(r"^\d+\.\s+", "", line)), styles["QBullet"]))
            i += 1
            continue
        if line.startswith("- "):
            story.append(Paragraph("• " + inline_markup(line[2:]), styles["QBullet"]))
            i += 1
            continue
        # Merge adjacent normal lines into a single paragraph.
        paragraph = [line]
        i += 1
        while i < len(lines) and lines[i].strip() and not lines[i].startswith(("#", "-", ">", "```", "|")) and lines[i].strip() != "---" and not re.match(r"^\d+\.\s+", lines[i]):
            paragraph.append(lines[i].strip())
            i += 1
        text = " ".join(paragraph)
        if text.startswith("**Document class:**"):
            story.extend([Spacer(1, 8), Paragraph(inline_markup(text), styles["QSubtitle"])])
        elif text.startswith("**Current status:**"):
            story.append(Paragraph(inline_markup(text), styles["QCallout"]))
        else:
            story.append(Paragraph(inline_markup(text), styles["QBody"]))
    # Add a deliberate cover break after the title block.
    story.insert(3, PageBreak())
    doc = PaperDoc(str(OUTPUT))
    doc.build(story)
    print(f"Created {OUTPUT} ({OUTPUT.stat().st_size} bytes)")

if __name__ == "__main__":
    main()
