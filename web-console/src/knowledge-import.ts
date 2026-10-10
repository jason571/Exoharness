const MAX_SOURCE_BYTES = 2 * 1024 * 1024;
const MAX_CONTENT_BYTES = 1024 * 1024;
const TEXT_EXTENSIONS = new Set(["txt", "md", "markdown", "csv", "json", "jsonl", "html", "htm"]);

export interface KnowledgeFileContent { title: string; content?: string; filename?: string; file_base64?: string; }

export function knowledgeImportWarnings(value: unknown): string[] {
  if (!value || typeof value !== "object" || !("warnings" in value) || !Array.isArray(value.warnings)) return [];
  const messages: Record<string, string> = {
    pdf_table_structure_unverified: "已导入版面文字，但 PDF 表格行列未验证，不会猜测或重建表格",
    page_without_text_layer: "部分页面没有文字层，未识别这些页面中的图片",
    images_not_interpreted: "已保存正文，内嵌图片未进行识别",
    external_links_not_fetched: "外部链接未访问，链接内容未导入",
    unsupported_word_block: "部分特殊 Word 内容不支持解析，请核对正文",
  };
  return value.warnings.map((code: unknown) => typeof code === "string" ? messages[code] || "部分内容存在解析限制，请核对资料" : "");
}

export async function readKnowledgeTextFile(file: File): Promise<KnowledgeFileContent> {
  const extension = file.name.split(".").pop()?.toLowerCase() || "";
  if (extension === "docx" || extension === "pdf") {
    if (!file.size || file.size > 4 * 1024 * 1024) throw new Error(`${file.name}：文件为空或超过 4 MB。`);
    const bytes = new Uint8Array(await file.arrayBuffer());
    const parts: string[] = [];
    for (let offset = 0; offset < bytes.length; offset += 32768) {
      parts.push(String.fromCharCode(...bytes.subarray(offset, offset + 32768)));
    }
    return { title: file.name, filename: file.name, file_base64: btoa(parts.join("")) };
  }
  if (!TEXT_EXTENSIONS.has(extension)) {
    throw new Error(`${file.name}：支持 DOCX、文字型 PDF、TXT、Markdown、CSV、JSON/JSONL 和 HTML；DOC 请先转换为 DOCX。`);
  }
  if (file.size > MAX_SOURCE_BYTES) throw new Error(`${file.name}：文件超过 2 MB，请拆分后再导入。`);
  let content: string;
  try {
    content = new TextDecoder("utf-8", { fatal: true }).decode(await file.arrayBuffer());
  } catch {
    throw new Error(`${file.name}：文件不是 UTF-8 编码，请转换后重试。`);
  }
  if (extension === "html" || extension === "htm") {
    const html = new DOMParser().parseFromString(content, "text/html");
    html.querySelectorAll("script, style, noscript, template").forEach((element) => element.remove());
    content = html.body.textContent || "";
  }
  content = content.replaceAll("\u0000", "").trim();
  if (!content) throw new Error(`${file.name}：未找到可检索的文字。`);
  if (new TextEncoder().encode(content).byteLength > MAX_CONTENT_BYTES) {
    throw new Error(`${file.name}：提取的文字超过 1 MB，请拆分后再导入。`);
  }
  const title = Array.from(file.name.trim()).slice(0, 160).join("");
  if (!title || Array.from(title).some((character) => /[\u0000-\u001f\u007f]/.test(character))) {
    throw new Error("文件名无效，请重命名后重试。");
  }
  return { title, content };
}
