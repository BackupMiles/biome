use crate::execute::diagnostics::{SearchDiagnostic, ResultExt};
use crate::execute::process_file::workspace_file::WorkspaceFile;
use crate::execute::process_file::{FileResult, FileStatus, Message, SharedTraversalOptions};
use biome_diagnostics::{category, DiagnosticExt, Severity};
use biome_service::workspace::PatternId;
use std::path::Path;

pub(crate) fn search<'ctx>(
    ctx: &'ctx SharedTraversalOptions<'ctx, '_>,
    path: &Path,
    pattern: &PatternId,
) -> FileResult {
    let mut workspace_file = WorkspaceFile::new(ctx, path)?;
    search_with_guard(ctx, &mut workspace_file, pattern)
}

pub(crate) fn search_with_guard<'ctx>(
    _ctx: &'ctx SharedTraversalOptions<'ctx, '_>,
    workspace_file: &mut WorkspaceFile,
    pattern: &PatternId,
) -> FileResult {
    tracing::info_span!("Processes searching", path =? workspace_file.path.display()).in_scope(
        move || {
            let result = workspace_file
                .guard()
                .search_pattern(pattern)
                .with_file_path_and_code(
                    workspace_file.path.display().to_string(),
                    category!("search"),
                )?;

            let input = workspace_file.input()?;
            let file_name = workspace_file.path.display().to_string();

            // FIXME: We need to report some real results here...
            let search_diagnostic = Message::Diagnostics {
                name: file_name,
                content: input,
                diagnostics: result
                    .matches
                    .into_iter()
                    .map(|mat| SearchDiagnostic.with_file_span(mat).with_severity(Severity::Information).into())
                    .collect(),
                skipped_diagnostics: 0,
            };
            Ok(FileStatus::Message(search_diagnostic))
        },
    )
}
