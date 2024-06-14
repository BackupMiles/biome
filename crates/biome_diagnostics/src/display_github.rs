use crate::display::frame::{IntoIter, SourceFile};
use crate::display::PrintHeader;
use crate::{diagnostic::internal::AsDiagnostic, Diagnostic, Resource, Severity};
use biome_console::{fmt, markup, MarkupBuf};
use biome_rowan::{TextRange};
use std::io;

/// Helper struct for printing a diagnostic as markup into any formatter
/// implementing [biome_console::fmt::Write].
pub struct PrintGitHubDiagnostic<'fmt, D: ?Sized>(pub &'fmt D);

impl<D: AsDiagnostic + ?Sized> fmt::Display for PrintGitHubDiagnostic<'_, D> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> io::Result<()> {
        let diagnostic = self.0.as_diagnostic();
        let location = diagnostic.location();

        // Docs:
        // https://docs.github.com/en/actions/using-workflows/workflow-commands-for-github-actions

        let Some(span) = location.span else {
            return Ok(());
        };
        let Some(source_code) = location.source_code else {
            return Ok(());
        };

        let file_name_unescaped = match &location.resource {
            Some(Resource::File(file)) => file,
            _ => return Ok(()),
        };

        let source = SourceFile::new(source_code);
        let start = source.location(span.start())?;
        let end = source.location(span.end())?;

        let command = match diagnostic.severity() {
            Severity::Error | Severity::Fatal => "error",
            Severity::Warning => "warning",
            Severity::Hint | Severity::Information => "notice",
        };

        let message = {
            let mut message = MarkupBuf::default();
            let mut fmt = fmt::Formatter::new(&mut message);
            fmt.write_markup(markup!({ PrintDiagnosticMessage(diagnostic) }))?;
            markup_to_string(&message)
        };

        let title = {
            diagnostic
                .category()
                .map(|category| category.name())
                .unwrap_or_default()
        };

        fmt.write_str(
            format! {
                "::{} title={},file={},line={},endLine={},col={},endColumn={}::{}",
                command, // constant, doesn't need escaping
                title, // the diagnostic category
                escape_property(file_name_unescaped),
                start.line_number, // integer, doesn't need escaping
                end.line_number, // integer, doesn't need escaping
                start.column_number, // integer, doesn't need escaping
                end.column_number, // integer, doesn't need escaping
                message.map_or_else(String::new, escape_data),
            }
            .as_str(),
        )?;

        Ok(())
    }
}

pub struct PrintMatchDiagnostic<'fmt, D: ?Sized> {
    diag: &'fmt D,
}

impl<'fmt, D: AsDiagnostic + ?Sized> PrintMatchDiagnostic<'fmt, D> {
    pub fn simple(diag: &'fmt D) -> Self {
        Self { diag }
    }
}

impl<D: AsDiagnostic + ?Sized> fmt::Display for PrintMatchDiagnostic<'_, D> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> io::Result<()> {
        let diagnostic = self.diag.as_diagnostic();
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
            let current_start = source.line_start(line_index.to_zero_indexed())?;
            let current_end = source.line_start(line_index.to_zero_indexed() + 1)?;

            // TODO: this is not removing <trailing blank lines>

            let current_range = TextRange::new(current_start, current_end);
            let current_text = source_code.text[current_range].trim_end_matches(['\r', '\n']);

            // write!(fmt, "{}:{}:    ", line_index, start.column_number)?;

            let mut current_iter = current_text.char_indices().into_iter();

            let is_first_line = line_index == start.line_number;
            let is_last_line = line_index == end.line_number;

            let start_index_relative_to_line = span.start().max(current_range.start()) - current_range.start();
            let end_index_relative_to_line = span.end().min(current_range.end()) - current_range.start();

            let marker = TextRange::new(start_index_relative_to_line, end_index_relative_to_line);

            // let marker = if is_first_line && is_last_line {
            //     Some(TextRange::new(
            //         start_index_relative_to_line,
            //         end_index_relative_to_line,
            //     ))
            // } else if is_first_line {
            //     Some(TextRange::new(
            //         start_index_relative_to_line,
            //         current_text.text_len()
            //     ))
            // } else if is_last_line {
            //     let start_index = current_text
            //         .text_len()
            //         .checked_sub(current_text.trim_start().text_len())
            //         .expect("integer overflow");
            //     Some(TextRange::new(
            //         start_index,
            //         end_index_relative_to_line
            //     ))
            // } else {
            //     None
            // };

            print_invisibles(
                fmt,
                current_text,
                &marker,
                PrintInvisiblesOptions {
                    ignore_trailing_carriage_return: true,
                    ignore_leading_tabs: true,
                    ignore_lone_spaces: true,
                    at_line_start: true,
                    at_line_end: true,
                },
            )?;

            write!(fmt, "\n")?;
        }

        write!(fmt, "\n\n")?;

        Ok(())
    }
}

pub(super) struct PrintInvisiblesOptions {
    /// Do not print tab characters at the start of the string
    pub(super) ignore_leading_tabs: bool,
    /// If this is set to true, space characters will only be substituted when
    /// at least two of them are found in a row
    pub(super) ignore_lone_spaces: bool,
    /// Do not print `'\r'` characters if they're followed by `'\n'`
    pub(super) ignore_trailing_carriage_return: bool,
    // Set to `true` to show invisible characters at the start of the string
    pub(super) at_line_start: bool,
    // Set to `true` to show invisible characters at the end of the string
    pub(super) at_line_end: bool,
}

/// Print `input` to `fmt` with invisible characters replaced with an
/// appropriate visual representation. Return `true` if any non-whitespace
/// character was printed
pub(super) fn print_invisibles(
    fmt: &mut fmt::Formatter<'_>,
    input: &str,
    range: &TextRange,
    options: PrintInvisiblesOptions,
) -> io::Result<bool> {
    let mut had_non_whitespace = false;

    // Get the first trailing whitespace character in the string
    let trailing_whitespace_index = input
        .char_indices()
        .rev()
        .find_map(|(index, char)| {
            if !char.is_ascii_whitespace() {
                Some(index)
            } else {
                None
            }
        })
        .unwrap_or(input.len());

    let mut iter = input.char_indices().peekable();
    let mut prev_char_was_whitespace = false;

    while let Some((i, char)) = iter.next() {
        let should_highlight
        let mut show_invisible = true;

        // Only highlight spaces when surrounded by other spaces
        if char == ' ' && options.ignore_lone_spaces {
            show_invisible = false;

            let next_char_is_whitespace = iter
                .peek()
                .map_or(false, |(_, char)| char.is_ascii_whitespace());

            if prev_char_was_whitespace || next_char_is_whitespace {
                show_invisible = false;
            }
        }

        prev_char_was_whitespace = char.is_ascii_whitespace();

        // Don't show leading tabs
        if options.at_line_start
            && !had_non_whitespace
            && char == '\t'
            && options.ignore_leading_tabs
        {
            show_invisible = false;
        }

        // Always show if at the end of line
        if options.at_line_end && i >= trailing_whitespace_index {
            show_invisible = true;
        }

        // If we are a carriage return next to a \n then don't show the character as visible
        if options.ignore_trailing_carriage_return && char == '\r' {
            let next_char_is_line_feed = iter.peek().map_or(false, |(_, char)| *char == '\n');
            if next_char_is_line_feed {
                continue;
            }
        }

        if !show_invisible {
            if !char.is_ascii_whitespace() {
                had_non_whitespace = true;
            }

            write!(fmt, "{char}")?;
            continue;
        }

        if let Some(visible) = show_invisible_char(char) {
            fmt.write_markup(markup! { <Dim>{visible}</Dim> })?;
            continue;
        }

        if (char.is_whitespace() && !char.is_ascii_whitespace()) || char.is_control() {
            let code = u32::from(char);
            fmt.write_markup(markup! { <Inverse>"U+"{format_args!("{code:x}")}</Inverse> })?;
            continue;
        }


        write!(fmt, "{char}")?;
    }

    Ok(had_non_whitespace)
}

fn show_invisible_char(char: char) -> Option<&'static str> {
    match char {
        ' ' => Some("\u{b7}"),      // Middle Dot
        '\r' => Some("\u{240d}"),   // Carriage Return Symbol
        '\n' => Some("\u{23ce}"),   // Return Symbol
        '\t' => Some("\u{2192} "),  // Rightwards Arrow
        '\0' => Some("\u{2400}"),   // Null Symbol
        '\x0b' => Some("\u{240b}"), // Vertical Tabulation Symbol
        '\x08' => Some("\u{232b}"), // Backspace Symbol
        '\x0c' => Some("\u{21a1}"), // Downards Two Headed Arrow
        _ => None,
    }
}

struct PrintDiagnosticMessage<'fmt, D: ?Sized>(&'fmt D);

impl<D: Diagnostic + ?Sized> fmt::Display for PrintDiagnosticMessage<'_, D> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> io::Result<()> {
        let Self(diagnostic) = *self;
        diagnostic.message(fmt)?;
        Ok(())
    }
}

fn escape_data<S: AsRef<str>>(value: S) -> String {
    let value = value.as_ref();

    // Refs:
    // - https://github.com/actions/runner/blob/a4c57f27477077e57545af79851551ff7f5632bd/src/Runner.Common/ActionCommand.cs#L18-L22
    // - https://github.com/actions/toolkit/blob/fe3e7ce9a7f995d29d1fcfd226a32bca407f9dc8/packages/core/src/command.ts#L80-L94
    let mut result = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\r' => result.push_str("%0D"),
            '\n' => result.push_str("%0A"),
            '%' => result.push_str("%25"),
            _ => result.push(c),
        }
    }
    result
}

fn escape_property<S: AsRef<str>>(value: S) -> String {
    let value = value.as_ref();

    // Refs:
    // - https://github.com/actions/runner/blob/a4c57f27477077e57545af79851551ff7f5632bd/src/Runner.Common/ActionCommand.cs#L25-L32
    // - https://github.com/actions/toolkit/blob/fe3e7ce9a7f995d29d1fcfd226a32bca407f9dc8/packages/core/src/command.ts#L80-L94
    let mut result = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\r' => result.push_str("%0D"),
            '\n' => result.push_str("%0A"),
            ':' => result.push_str("%3A"),
            ',' => result.push_str("%2C"),
            '%' => result.push_str("%25"),
            _ => result.push(c),
        }
    }
    result
}

fn markup_to_string(markup: &MarkupBuf) -> Option<String> {
    let mut buffer = Vec::new();
    let mut write = fmt::Termcolor(termcolor::NoColor::new(&mut buffer));
    let mut fmt = fmt::Formatter::new(&mut write);
    fmt.write_markup(markup! { {markup} }).ok()?;
    String::from_utf8(buffer).ok()
}
