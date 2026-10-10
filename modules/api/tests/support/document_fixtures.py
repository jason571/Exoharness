"""Generated non-sensitive fixtures and tests, executed in the existing OCI image."""
import base64
import io
import json
import subprocess
import sys
import unittest
import zipfile
from pathlib import Path

NAMESPACE = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"


def docx(rows=3, external=False):
    content = ('<w:p><w:r><w:t>销售资料：单位为万元</w:t></w:r></w:p>'
               '<w:tbl><w:tr><w:trPr><w:tblHeader/></w:trPr>'
               '<w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t>名称与金额（万元）</w:t></w:r></w:p></w:tc></w:tr>'
               '<w:tr><w:tc><w:tcPr><w:vMerge w:val="restart"/></w:tcPr><w:p><w:r><w:t>农业</w:t></w:r></w:p></w:tc>'
               '<w:tc><w:p><w:r><w:t>100</w:t></w:r></w:p></w:tc></w:tr>'
               '<w:tr><w:tc><w:tcPr><w:vMerge/></w:tcPr><w:p/></w:tc>'
               '<w:tc><w:p><w:r><w:t>200</w:t></w:r></w:p></w:tc></w:tr>')
    for i in range(3, rows):
        content += f'<w:tr><w:tc><w:p><w:r><w:t>产品{i}</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>{i}</w:t></w:r></w:p></w:tc></w:tr>'
    content += '</w:tbl><w:p><w:r><w:t>表后说明</w:t></w:r></w:p>'
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("[Content_Types].xml", '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>')
        z.writestr("_rels/.rels", '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>')
        z.writestr("word/document.xml", f'<w:document xmlns:w="{NAMESPACE}"><w:body>{content}</w:body></w:document>')
        if external:
            z.writestr("word/_rels/document.xml.rels", '<Relationships><Relationship TargetMode="External" Target="https://example.invalid/private"/></Relationships>')
    return output.getvalue()


def pdf(blank=False, encrypted=False, pages=1):
    from pypdf import PdfWriter
    from pypdf.generic import DictionaryObject, NameObject, DecodedStreamObject
    writer = PdfWriter()
    page = writer.add_blank_page(600, 800)
    if not blank:
        font = DictionaryObject({NameObject("/Type"): NameObject("/Font"),
                                 NameObject("/Subtype"): NameObject("/Type1"),
                                 NameObject("/BaseFont"): NameObject("/Helvetica")})
        page[NameObject("/Resources")] = DictionaryObject({NameObject("/Font"): DictionaryObject({NameObject("/F1"): writer._add_object(font)})})
        stream = DecodedStreamObject()
        stream.set_data(b"BT /F1 12 Tf 40 700 Td (Product  Price USD) Tj 0 -20 Td (Tea      100) Tj ET")
        page[NameObject("/Contents")] = writer._add_object(stream)
    if encrypted:
        writer.encrypt("test-only-password")
    for _ in range(1, pages):
        writer.add_page(page)
    output = io.BytesIO()
    writer.write(output)
    return output.getvalue()


class ParserTests(unittest.TestCase):
    def parse(self, data, extension):
        Path(f"input.{extension}").write_bytes(data)
        process = subprocess.run([sys.executable, "parse_document.py", extension], capture_output=True, text=True, timeout=30, check=True)
        return json.loads(process.stdout)

    def test_chinese_order_merged_cells_headers_units(self):
        result = self.parse(docx(), "docx")
        self.assertEqual([b["kind"] for b in result["blocks"]], ["paragraph", "table", "paragraph"])
        table = result["blocks"][1]
        self.assertEqual(table["row_count"], 3)
        self.assertEqual(table["rows"][0][0]["colspan"], 2)
        self.assertEqual(table["rows"][1][0]["rowspan"], 2)
        self.assertEqual(table["header_rows"], [0])
        self.assertIn("万元", table["rows"][0][0]["text"])

    def test_large_table_is_one_block(self):
        result = self.parse(docx(200), "docx")
        self.assertEqual(len(result["blocks"]), 3)
        self.assertEqual(result["blocks"][1]["row_count"], 200)

    def test_external_reference_not_fetched(self):
        self.assertIn("external_links_not_fetched", self.parse(docx(external=True), "docx")["warnings"])

    def test_text_pdf_layout_without_guessed_cells(self):
        result = self.parse(pdf(pages=2), "pdf")
        self.assertIn("pdf_table_structure_unverified", result["warnings"])
        self.assertEqual(result["blocks"][0]["kind"], "layout")
        self.assertIn("Price USD", result["content"])
        self.assertNotIn("rows", result["blocks"][0])
        self.assertEqual(len(result["blocks"]), 1)
        self.assertEqual(len(result["blocks"][0]["pages"]), 2)

    def test_pdf_without_text_layer(self):
        self.assertEqual(self.parse(pdf(blank=True), "pdf")["error"], "pdf_without_text_layer")

    def test_encrypted_pdf(self):
        self.assertEqual(self.parse(pdf(encrypted=True), "pdf")["error"], "encrypted_pdf")

    def test_broken_documents(self):
        for extension in ("docx", "pdf"):
            self.assertEqual(self.parse(b"broken", extension)["error"], "invalid_document")

    def test_zip_bomb_and_xml_entity(self):
        for value, expected in [(b"A" * 1024 * 1024, "unsafe_package"),
                                (b'<!DOCTYPE x [<!ENTITY a SYSTEM "file:///etc/passwd">]><x/>', "unsafe_xml")]:
            output = io.BytesIO()
            with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED) as z:
                z.writestr("word/document.xml", value)
            self.assertEqual(self.parse(output.getvalue(), "docx")["error"], expected)


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "fixtures":
        print(json.dumps({"docx": base64.b64encode(docx(200)).decode(),
                          "pdf": base64.b64encode(pdf()).decode(),
                          "blank_pdf": base64.b64encode(pdf(blank=True)).decode()}))
    else:
        unittest.main()
