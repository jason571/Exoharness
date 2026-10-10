"""Trusted, network-free document reader. Uploaded bytes are data, never code."""
import json
import sys
import zipfile
from pathlib import PurePosixPath
from xml.etree import ElementTree as ET

MAX_TEXT = 1024 * 1024
W = "{http://schemas.openxmlformats.org/wordprocessingml/2006/main}"


def xml(data):
    probe = data.replace(b"\0", b"").upper()
    if b"<!DOCTYPE" in probe or b"<!ENTITY" in probe:
        raise ValueError("unsafe_xml")
    return ET.fromstring(data)


def text(node):
    return "".join((n.text or "") if n.tag == W + "t" else
                   "\t" if n.tag == W + "tab" else
                   "\n" if n.tag in (W + "br", W + "cr") else ""
                   for n in node.iter())


def table(node, ordinal):
    rows = []
    previous = {}
    cells = []
    for r, tr in enumerate(node.findall(W + "tr")):
        row = []
        column = 0
        before = tr.find(W + "trPr/" + W + "gridBefore")
        if before is not None:
            column = int(before.get(W + "val", "0"))
        if not 0 <= column <= 1024:
            raise ValueError("invalid_document")
        current = {}
        for tc in tr.findall(W + "tc"):
            span = tc.find(W + "tcPr/" + W + "gridSpan")
            width = int(span.get(W + "val", "1")) if span is not None else 1
            if not 1 <= width <= 1024 or column > 1024:
                raise ValueError("table_too_large")
            merge = tc.find(W + "tcPr/" + W + "vMerge")
            continuation = merge is not None and merge.get(W + "val") != "restart"
            if continuation:
                anchor = previous.get(column)
                if anchor is None or anchor["colspan"] != width:
                    raise ValueError("invalid_table_merge")
                anchor["rowspan"] += 1
                current[column] = anchor
            else:
                cell = {"row": r, "column": column, "colspan": width,
                        "rowspan": 1, "text": text(tc)}
                nested = tc.findall(W + "tbl")
                if nested:
                    cell["tables"] = [table(t, f"{ordinal}-nested-{r}-{column}-{i}")
                                      for i, t in enumerate(nested)]
                cells.append(cell)
                row.append(cell)
                if merge is not None:
                    current[column] = cell
            column += width
        rows.append(row)
        previous = current
    header_rows = []
    for i, tr in enumerate(node.findall(W + "tr")):
        header = tr.find(W + "trPr/" + W + "tblHeader")
        if header is not None and header.get(W + "val", "true") not in ("0", "false", "off"):
            header_rows.append(i)
    grid = [c.get(W + "w") for c in node.findall(W + "tblGrid/" + W + "gridCol")]
    caption = node.find(W + "tblPr/" + W + "tblCaption")
    return {"id": f"table-{ordinal}", "kind": "table", "rows": rows,
            "row_count": len(rows), "column_count": max(len(grid), max(
                (c["column"] + c["colspan"] for c in cells), default=0)),
            "grid_widths_twips": grid,
            "caption": caption.get(W + "val") if caption is not None else None,
            "header_rows": header_rows, "header_inferred": False,
            "units_inferred": False, "source": {"body_index": ordinal}}


def docx(path):
    warnings = []
    with zipfile.ZipFile(path) as archive:
        entries = archive.infolist()
        if len(entries) > 4096 or sum(e.file_size for e in entries) > 32 * 1024 * 1024:
            raise ValueError("package_too_large")
        names = [e.filename for e in entries]
        if len(set(names)) != len(names):
            raise ValueError("invalid_package")
        for entry in entries:
            if (entry.flag_bits & 1 or entry.file_size > 8 * 1024 * 1024
                    or entry.file_size > max(entry.compress_size, 1) * 200
                    or ".." in PurePosixPath(entry.filename).parts
                    or entry.filename.startswith("/") or "\\" in entry.filename):
                raise ValueError("unsafe_package")
            if "vbaproject" in entry.filename.lower():
                raise ValueError("macros_not_supported")
            if entry.filename.endswith(".rels"):
                relations = xml(archive.read(entry))
                if any(r.get("TargetMode") == "External" for r in relations):
                    warnings.append("external_links_not_fetched")
        root = xml(archive.read("word/document.xml"))
    body = root.find(W + "body")
    if body is None:
        raise ValueError("invalid_document")
    blocks = []
    for i, child in enumerate(body):
        if child.tag == W + "p":
            value = text(child)
            if value.strip():
                style = child.find(W + "pPr/" + W + "pStyle")
                heading = child.find(W + "pPr/" + W + "outlineLvl") is not None
                blocks.append({"id": f"text-{i}", "kind": "heading" if heading else "paragraph", "text": value,
                               "style": style.get(W + "val") if style is not None else None,
                               "source": {"body_index": i}})
        elif child.tag == W + "tbl":
            parsed = table(child, i)
            parsed["context_before"] = [b["text"][:2000] for b in blocks[-2:]
                                        if b["kind"] in ("paragraph", "heading")]
            blocks.append(parsed)
        elif child.tag != W + "sectPr":
            warnings.append("unsupported_word_block")
    if root.find(".//" + W + "drawing") is not None:
        warnings.append("images_not_interpreted")
    return blocks, sorted(set(warnings))


def pdf(path):
    from pypdf import PdfReader
    reader = PdfReader(path, strict=True)
    if reader.is_encrypted:
        raise ValueError("encrypted_pdf")
    if len(reader.pages) > 300:
        raise ValueError("too_many_pages")
    pages = []
    warnings = ["pdf_table_structure_unverified"]
    size = 0
    for index, page in enumerate(reader.pages):
        value = (page.extract_text(extraction_mode="layout") or "") if page.get("/Contents") is not None else ""
        size += len(value.encode("utf-8"))
        if size > MAX_TEXT:
            raise ValueError("extracted_text_too_large")
        pages.append({"page": index + 1, "text": value})
        if not value.strip():
            warnings.append("page_without_text_layer")
    if not any(p["text"].strip() for p in pages):
        raise ValueError("pdf_without_text_layer")
    # One atomic layout block: don't split a possible cross-page table or guess cells.
    return [{"id": "pdf-layout", "kind": "layout", "pages": pages,
             "source": {"first_page": 1, "last_page": len(pages)},
             "table_structure_verified": False}], sorted(set(warnings))


def block_text(block):
    if block["kind"] == "table":
        return "\n".join("\t".join(c["text"] for c in row) for row in block["rows"])
    if block["kind"] == "layout":
        return "\n".join(p["text"] for p in block["pages"])
    return block["text"]


def main():
    try:
        kind = sys.argv[1]
        blocks, warnings = docx("input.docx") if kind == "docx" else pdf("input.pdf")
        content = "\n\n".join(block_text(b) for b in blocks).replace("\0", "")
        if not content.strip():
            raise ValueError("no_text")
        if len(content.encode("utf-8")) > MAX_TEXT:
            raise ValueError("extracted_text_too_large")
        result = {"version": 1, "format": kind, "content": content,
                  "blocks": blocks, "warnings": warnings}
        encoded = json.dumps(result, ensure_ascii=False)
        if len(encoded.encode("utf-8")) > 1900 * 1024:
            raise ValueError("structured_document_too_large")
        print(encoded)
    except (ImportError, ModuleNotFoundError):
        print(json.dumps({"error": "parser_dependency_missing"}))
    except ValueError as error:
        safe = {"unsafe_xml", "table_too_large", "invalid_table_merge", "package_too_large",
                "invalid_package", "unsafe_package", "macros_not_supported", "invalid_document",
                "encrypted_pdf", "too_many_pages", "pdf_without_text_layer", "no_text",
                "extracted_text_too_large", "structured_document_too_large"}
        print(json.dumps({"error": str(error) if str(error) in safe else "invalid_document"}))
    except Exception:
        print(json.dumps({"error": "invalid_document"}))


if __name__ == "__main__":
    main()
