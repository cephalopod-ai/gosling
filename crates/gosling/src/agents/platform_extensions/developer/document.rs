use std::fs;
use std::path::Path;

use office_oxide::create::create_from_markdown;
use office_oxide::DocumentFormat;
use rmcp::model::{CallToolResult, Content};
use schemars::JsonSchema;
use serde::Deserialize;

use super::edit::resolve_path;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocumentWriteParams {
    /// Path of the document to create. Its extension selects the format: .docx, .xlsx, or .pptx.
    pub path: String,
    /// Markdown source. Headings, pipe tables, and lists are converted to the target format.
    pub content: String,
}

pub struct DocumentTool;

impl DocumentTool {
    pub fn new() -> Self {
        Self
    }

    pub fn write_document_with_cwd(
        &self,
        params: DocumentWriteParams,
        working_dir: Option<&Path>,
    ) -> CallToolResult {
        let path = resolve_path(&params.path, working_dir);

        let Some(format) = document_format(&path) else {
            return error(format!(
                "{} is not an Office document. write_document creates .docx, .xlsx, or .pptx; \
                 use write for text formats.",
                params.path
            ));
        };

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                if let Err(cause) = fs::create_dir_all(parent) {
                    return error(format!(
                        "Failed to create directory {}: {cause}",
                        parent.display()
                    ));
                }
            }
        }

        let existed = path.exists();
        if let Err(cause) = create_from_markdown(&params.content, format, &path) {
            return error(format!("Failed to write {}: {cause}", params.path));
        }

        let bytes = fs::metadata(&path).map(|data| data.len()).unwrap_or(0);
        let action = if existed { "Replaced" } else { "Created" };
        CallToolResult::success(vec![Content::text(format!(
            "{action} {} ({bytes} bytes)",
            params.path
        ))
        .with_priority(0.0)])
    }
}

impl Default for DocumentTool {
    fn default() -> Self {
        Self::new()
    }
}

/// office_oxide reads the legacy formats but can only create OOXML, so the
/// tool advertises exactly the three it can write.
fn document_format(path: &Path) -> Option<DocumentFormat> {
    match path
        .extension()
        .and_then(|value| value.to_str())?
        .to_ascii_lowercase()
        .as_str()
    {
        "docx" => Some(DocumentFormat::Docx),
        "xlsx" => Some(DocumentFormat::Xlsx),
        "pptx" => Some(DocumentFormat::Pptx),
        _ => None,
    }
}

fn error(message: String) -> CallToolResult {
    CallToolResult::error(vec![Content::text(message).with_priority(0.0)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use office_oxide::Document;

    fn write(path: &Path, content: &str) -> CallToolResult {
        DocumentTool::new().write_document_with_cwd(
            DocumentWriteParams {
                path: path.to_string_lossy().into_owned(),
                content: content.to_string(),
            },
            None,
        )
    }

    fn text_of(result: &CallToolResult) -> String {
        match &result.content[0].raw {
            rmcp::model::RawContent::Text(text) => text.text.clone(),
            _ => panic!("expected text content"),
        }
    }

    #[test]
    fn writes_a_readable_docx_from_markdown() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("report.docx");

        let result = write(&path, "# Findings\n\nThe sample held.\n");

        assert_eq!(result.is_error, Some(false));
        assert!(text_of(&result).starts_with("Created"));
        let document = Document::open(&path).unwrap();
        assert_eq!(document.format(), DocumentFormat::Docx);
        assert!(document.plain_text().contains("The sample held."));
    }

    #[test]
    fn writes_a_spreadsheet_from_a_markdown_table() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("costs.xlsx");

        let result = write(
            &path,
            "# Costs\n\n| Item | Amount |\n| --- | --- |\n| Launch | 42 |\n",
        );

        assert_eq!(result.is_error, Some(false));
        let document = Document::open(&path).unwrap();
        assert_eq!(document.format(), DocumentFormat::Xlsx);
        let text = document.plain_text();
        assert!(text.contains("Launch"));
        assert!(text.contains("42"));
    }

    #[test]
    fn creates_missing_parent_directories_and_reports_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("deliverables").join("deck.pptx");

        assert_eq!(write(&path, "# One\n\nFirst").is_error, Some(false));
        let replaced = write(&path, "# Two\n\nSecond");

        assert_eq!(replaced.is_error, Some(false));
        assert!(text_of(&replaced).starts_with("Replaced"));
        assert!(Document::open(&path)
            .unwrap()
            .plain_text()
            .contains("Second"));
    }

    #[test]
    fn rejects_formats_it_cannot_create() {
        let temp = tempfile::tempdir().unwrap();

        for name in ["notes.odt", "legacy.doc", "notes.md", "plain"] {
            let result = write(&temp.path().join(name), "# Heading");
            assert_eq!(result.is_error, Some(true), "{name} should be rejected");
            assert!(text_of(&result).contains(".docx, .xlsx, or .pptx"));
        }
    }
}
