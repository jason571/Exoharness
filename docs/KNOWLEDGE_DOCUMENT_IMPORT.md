# Knowledge classification, import and retrieval

English | [简体中文](KNOWLEDGE_DOCUMENT_IMPORT.zh-CN.md)

Users manage their own materials and memories. Administrators create shared libraries and grant them to specific assistants. Categories can be nested; imports can include a category and tags. Document content is reference data and cannot grant tool permission or override system rules.

## Formats

Text, `.docx` and text-based PDFs are supported. Convert legacy `.doc` first. Scanned PDFs, image OCR, encrypted or damaged files are outside the current scope. Embedded images are not recognized automatically, and a file with no extracted text is not marked as searchable.

DOCX and PDF parsing runs in a separate network-disabled sandbox with fixed parsers. Only document content is parsed; macros, external links and uploaded code remain disabled. Import errors are recorded; readiness is set only after successful extraction.

## Tables and provenance

DOCX structural blocks preserve body order, rows and columns, explicit headers, merged-cell anchors and source positions. A complete table remains one structural block; it is not split at a fixed character or Token count.

Very large tables can be read on demand by block ID and revision. Partial views retain provenance, row numbers and header associations, and identify the read range with `partial`. Headers and units follow the source document's structure.

PDF currently preserves text layout and page provenance; it does **not** reliably identify semantic table rows and columns. It can return warnings such as `pdf_table_structure_unverified`. A conservative layout block does not guarantee reconstruction of a table spanning pages; cell relationships are not guessed.

Originals reside in a private sandbox workspace. The database records the path, filename, size, SHA-256, revision and extraction result. Deleting a document removes its registry and structures; physical retention of originals and parse evidence follows a separate policy.

## API and limits

`POST /v1/knowledge/documents/import` accepts `filename`, `file_base64`, `category_id` and `tags`; Base64 is only the HTTP transfer encoding. The server assigns the owner and document ID.

`GET .../documents/{id}/blocks` returns a compact index and warnings. `GET .../blocks/{block}?revision=1&offset=0&limit=100` reads a versioned range. Shared administrator libraries use the corresponding `/v1/knowledge/libraries/{library}/documents/...` routes.

DOCX/PDF source files are limited to 4 MiB, extracted text to 1 MiB and PDFs to 300 pages. Decompression is limited to 32 MiB and validates entry count, paths, compression ratio, entities and macros. Parsing defaults to at most two concurrent jobs, 60 seconds per job, 512 MiB and one CPU.

## Retrieval and memory status

Owner-scoped literal search and structural reads are implemented. Shared-library grants, categories and personal-memory management are connected. **pgvector, semantic embedding indexes and automatic Agent retrieval of materials/memories are not connected**; the system does not promise that every conversation automatically uses imported documents.

Personal memories store user-managed preferences and are isolated by user. Shared administrator libraries are retrieved according to assistant grants. Retrieved documents and API text are treated as data.

Implementation is in `api/src/knowledge/`, PostgreSQL storage access and incremental migrations; the parser is `api/src/knowledge/imports/parse_document.py`. Tests are in `modules/api/tests/knowledge.rs` and `scripts/tests/test_document_parser.py`.
