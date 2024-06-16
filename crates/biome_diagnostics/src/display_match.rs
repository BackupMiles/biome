use crate::display::frame::{IntoIter, SourceFile};
use crate::display::PrintHeader;
use crate::diagnostic::internal::AsDiagnostic;
use crate::Diagnostic;
use biome_console::{fmt, markup};
use biome_rowan::{TextLen, TextRange, TextSize};
use std::io;

pub struct PrintMatchDiagnostic<'fmt, D: ?Sized>(pub &'fmt D);

impl<D: AsDiagnostic + ?Sized> fmt::Display for PrintMatchDiagnostic<'_, D> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> io::Result<()> {
        let diagnostic = self.0.as_diagnostic();
        let location = diagnostic.location();

        let Some(span) = location.span else {
            return Ok(());
        };
        let Some(source_code) = location.source_code else {
            return Ok(());
        };

        fmt.write_markup(markup! {
            {PrintHeader(diagnostic)}"\n\n"
        })?;

        let source = SourceFile::new(source_code);
        
        let start = source.location(span.start())?;
        let end = source.location(span.end())?;

        let match_line_start = start.line_number;
        let match_line_end = end.line_number.saturating_add(1);

        for line_index in IntoIter::new(match_line_start..match_line_end) {
            let current_range = source.line_range(line_index.to_zero_indexed());
            let current_range = match current_range {
                Ok(v) => v,
                Err(_) => continue
            };

            let current_text = source_code.text[current_range].trim_end_matches(['\r', '\n']);

            let is_first_line = line_index == start.line_number;
            let is_last_line = line_index == end.line_number;

            let start_index_relative_to_line = span.start().max(current_range.start()) - current_range.start();
            let end_index_relative_to_line = span.end().min(current_range.end()) - current_range.start();

            let marker = if is_first_line && is_last_line {
                TextRange::new(
                    start_index_relative_to_line,
                    end_index_relative_to_line,
                )
            // TODO: can this become the most generic else block? The logic might be there
            } else if is_first_line {
                TextRange::new(
                    start_index_relative_to_line,
                    current_text.text_len()
                )
            } else if is_last_line {
                let start_index = current_text
                    .text_len()
                    .checked_sub(current_text.trim_start().text_len())
                    .expect("integer overflow");
                TextRange::new(
                    start_index,
                    end_index_relative_to_line
                )
            } else {
                // whole line
                TextRange::new(TextSize::from(0), current_text.text_len())
            };

            fmt.write_markup(markup! {
                <Emphasis>{format_args!("{line_index} \u{2502} ")}</Emphasis>
            })?;

            let mut iter = current_text.char_indices().peekable();

            while let Some((i, char)) = iter.next() {
                let should_highlight = i >= marker.start().into() && i < marker.end().into();
                if should_highlight {
                    fmt.write_markup(markup! { <Emphasis><Info>{char}</Info></Emphasis> })?;
                    continue;
                }

                write!(fmt, "{char}")?;
            }

            write!(fmt, "\n")?;
        }

        Ok(())
    }
}
