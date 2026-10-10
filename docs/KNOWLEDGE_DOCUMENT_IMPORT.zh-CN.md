# 资料分类 导入与检索

[English](KNOWLEDGE_DOCUMENT_IMPORT.md) | 简体中文

用户管理自己的资料和记忆；管理员建立公共资料库并授予指定助理。分类按路径分层，导入时可选择分类及标签。文档内容是参考数据，不能授予工具权限或覆盖系统规则。

## 格式

支持文本类导入与 `.docx`、文字型 PDF。旧 `.doc` 应先转换；扫描件 PDF、图片 OCR 和加密/损坏文档不在当前范围。内嵌图片不自动识别，不把无文字的文件标记为可检索成功。

DOCX/PDF 在独立断网沙盒中使用固定解析器处理。解析范围为文档内容，宏、外链和上传代码保持禁用。导入失败记录错误，成功提取后才登记就绪状态。

## 表格与来源

DOCX 结构块保留正文顺序、行列、明确表头、合并单元格锚点及原文位置。完整表格是一个结构块，不按固定字符数或 Token 切开。

超大表格通过 block ID 与 revision 按需读取。局部视图保留来源、行号和表头关联，并以 `partial` 标记读取范围。表头和单位以原始文档结构为依据。

PDF 当前保存文字布局及页来源，**没有可靠的语义行列表格识别**；返回 `pdf_table_structure_unverified` 等警告。保守布局块不意味着天然还原跨页表格，不猜测单元格关系。

原文件保存在私有 Sandbox 工作区。数据库记录路径、名称、大小、SHA-256、revision 和提取结果。删除资料时清理登记与结构；原文件和解析证据的物理清理由独立保留策略管理。

## 接口与限制

`POST /v1/knowledge/documents/import` 输入 `filename`、`file_base64`、`category_id`、`tags`；Base64 只是 HTTP 传输编码。owner 与文档 ID 由服务器确定。

`GET .../documents/{id}/blocks` 取得紧凑清单和警告；`GET .../blocks/{block}?revision=1&offset=0&limit=100` 按版本读取。管理员公共库使用对应的 `/v1/knowledge/libraries/{library}/documents/...` 路径。

DOCX/PDF 原件最大 4 MiB，提取文字最大 1 MiB，PDF 最多 300 页；解压限制 32 MiB 并检查条目、路径、压缩比、实体和宏。解析默认最多并发 2、单次 60 秒、512 MiB、1 CPU。

## 检索和记忆状态

已实现 owner 限定的字面检索和结构读取。公共库授权、分类和个人记忆管理已接通；**pgvector、语义 embedding 索引、Agent 自动检索资料/记忆尚未接通**，不能承诺每次用户对话都会自动利用已导入资料。

个人记忆保存用户可管理的偏好记录，按用户隔离。管理员资料库按助理授权检索，资料和 API 返回文本作为检索数据处理。

实现：`api/src/knowledge/`、`storage/postgres` 资料访问和增量迁移；解析器在 `api/src/knowledge/imports/parse_document.py`。测试见 `modules/api/tests/knowledge.rs` 与 `scripts/tests/test_document_parser.py`。
