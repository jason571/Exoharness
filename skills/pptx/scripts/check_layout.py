"""Read-only PPT geometry checks; not a claim of visual or factual correctness."""
import argparse
import json
import math
import unicodedata
import zipfile
from collections import defaultdict

from pptx import Presentation
from pptx.enum.shapes import MSO_SHAPE_TYPE

EMU = 914400


def broken_text_code(text):
    """Return a diagnostic for damaged text; never rewrite the supplied copy.

    Preserve normal Unicode and common whitespace. Only recognize mojibake when
    a Latin-1/Windows-1252 run reverses exactly to valid UTF-8 containing CJK.
    Accented text alone is not evidence of a damaged encoding.
    """
    if '\ufffd' in text:
        return 'replacement_character'
    if any(0xd800 <= ord(char) <= 0xdfff for char in text):
        return 'invalid_unicode_surrogate'
    if any((ord(char) < 32 and char not in '\t\n\r')
           or 0x7f <= ord(char) <= 0x9f for char in text):
        return 'control_character'

    def reversed_cjk(run, encoding):
        if not run or all(char.isascii() for char in run):
            return False
        try:
            repaired = ''.join(run).encode(encoding).decode('utf-8')
        except (UnicodeEncodeError, UnicodeDecodeError):
            return False
        return any(unicodedata.name(char, '').startswith(
            ('CJK UNIFIED IDEOGRAPH-', 'CJK COMPATIBILITY IDEOGRAPH-'))
            for char in repaired)

    for encoding in ('latin-1', 'cp1252'):
        run = []
        for char in text:
            try:
                char.encode(encoding)
            except UnicodeEncodeError:
                if reversed_cjk(run, encoding):
                    return 'reversible_cjk_mojibake'
                run = []
            else:
                run.append(char)
        if reversed_cjk(run, encoding):
            return 'reversible_cjk_mojibake'
    return None


def inspect(path, no_images=False, table_overlap=False):
    with zipfile.ZipFile(path) as archive:
        if len(archive.infolist()) > 16000 or sum(x.file_size for x in archive.infolist()) > 128 * 1024 * 1024:
            raise ValueError("presentation exceeds inspection size limit")
    deck = Presentation(path)
    errors, warnings = [], []
    width, height = deck.slide_width / EMU, deck.slide_height / EMU

    def shapes(items, transform=(0, 0, 1, 1), rotated=False):
        ox, oy, sx, sy = transform
        for shape in items:
            x, y = ox + shape.left / EMU * sx, oy + shape.top / EMU * sy
            w, h = shape.width / EMU * sx, shape.height / EMU * sy
            rotation = rotated or bool(shape.rotation)
            if shape.shape_type == MSO_SHAPE_TYPE.GROUP:
                xf = shape._element.grpSpPr.xfrm
                if xf is not None and xf.chExt is not None and xf.chExt.cx and xf.chExt.cy:
                    gx, gy = w * EMU / xf.chExt.cx, h * EMU / xf.chExt.cy
                    yield from shapes(shape.shapes, (x - xf.chOff.x / EMU * gx, y - xf.chOff.y / EMU * gy, gx, gy), rotation)
                else:
                    warnings.append({"slide": page, "shape": shape.shape_id, "code": "unresolved_group_transform"})
                continue
            yield shape, (x, y, w, h), rotation

    for page, slide in enumerate(deck.slides, 1):
        text_boxes = []
        table_boxes = []
        for shape, box, rotated in shapes(slide.shapes):
            if table_overlap and shape.has_table and not rotated:
                x, y, w, h = box
                # Some generators leave a 1-inch graphicFrame while row heights
                # span several inches. Office renders the rows, not that stub.
                sx = w * EMU / shape.width if shape.width else 1
                sy = h * EMU / shape.height if shape.height else 1
                tw = max(w, sum(col.width for col in shape.table.columns) / EMU * sx)
                th = max(h, sum(row.height for row in shape.table.rows) / EMU * sy)
                table_boxes.append((shape, (x, y, tw, th)))
            if no_images and shape.shape_type in (MSO_SHAPE_TYPE.PICTURE, MSO_SHAPE_TYPE.LINKED_PICTURE):
                errors.append({"slide": page, "shape": shape.shape_id, "code": "image_not_requested"})
            if not shape.has_text_frame or not shape.text_frame.text.strip():
                continue
            x, y, w, h = box
            if not all(math.isfinite(n) for n in box) or w <= 0 or h <= 0:
                errors.append({"slide": page, "shape": shape.shape_id, "code": "invalid_text_dimensions"})
            elif rotated:
                warnings.append({"slide": page, "shape": shape.shape_id, "code": "rotated_text_needs_visual_review"})
            elif x < -0.03 or y < -0.03 or x + w > width + 0.03 or y + h > height + 0.03:
                errors.append({"slide": page, "shape": shape.shape_id, "code": "text_outside_slide", "box_inches": [round(v, 3) for v in box]})
            if not rotated:
                text_boxes.append((shape, box))

        # Opt-in for newly authored decks. Templates may intentionally place
        # separate labels over a table; default/template checks are unchanged.
        for shape, (x, y, w, h) in text_boxes:
            for table, (tx, ty, tw, th) in table_boxes:
                ix = min(x + w, tx + tw) - max(x, tx)
                iy = min(y + h, ty + th) - max(y, ty)
                if ix > .03 and iy > .03:
                    errors.append({"slide": page, "shape": shape.shape_id,
                                   "table": table.shape_id, "code": "text_overlaps_table",
                                   "text_box_inches": [round(v, 3) for v in (x, y, w, h)],
                                   "table_box_inches": [round(v, 3) for v in (tx, ty, tw, th)]})

        # A mass of identical text rectangles catches the dropped-options bug.
        # Two deliberately layered labels/shadows and shape backgrounds are not
        # errors; neither are chart labels or merely intersecting text boxes.
        coincident = defaultdict(list)
        for shape, box in text_boxes:
            coincident[tuple(round(v, 3) for v in box)].append(shape.shape_id)
        for box, ids in coincident.items():
            if len(ids) >= 3 and len(ids) * 2 >= len(text_boxes):
                errors.append({"slide": page, "code": "collapsed_text_geometry", "shapes": ids[:12], "count": len(ids), "box_inches": list(box)})

    return {"status": "failed" if errors else "passed", "slides": len(deck.slides),
            "error_count": len(errors), "errors": errors[:24], "warning_count": len(warnings), "warnings": warnings[:12],
            "scope": "Geometry and optional image policy only; not font fit, visual aesthetics or factual verification."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pptx")
    parser.add_argument("--no-images", action="store_true", help="Reject image shapes only when the user requested an image-free deck")
    parser.add_argument("--table-overlap", action="store_true", help="For new decks, reject separate text boxes overlapping tables; omit for intentional template overlays")
    args = parser.parse_args()
    try:
        result = inspect(args.pptx, args.no_images, args.table_overlap)
    except (OSError, ValueError, zipfile.BadZipFile) as error:
        result = {"status": "failed", "error": str(error)[:300]}
    print(json.dumps(result, ensure_ascii=False))
    return 0 if result["status"] == "passed" else 2


if __name__ == "__main__":
    raise SystemExit(main())
