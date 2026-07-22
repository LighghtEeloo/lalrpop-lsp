use std::sync::Arc;

use lalrpop::lsp::{DiagnosticError, ErrorLoc, LalrpopFile, Span, SpanItem, TypeDecl};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

use crate::line_index::LineIndex;

#[derive(Clone, Debug)]
struct SourceText {
    text: Arc<str>,
    line_index: Arc<LineIndex>,
}

impl SourceText {
    fn new(text: String) -> Self {
        let text: Arc<str> = Arc::from(text);
        let line_index = Arc::new(LineIndex::new(&text));
        Self { text, line_index }
    }

    fn as_str(&self) -> &str {
        &self.text
    }

    fn offset(&self, position: Position) -> Option<usize> {
        self.line_index.offset(self.as_str(), position)
    }

    fn position(&self, offset: usize) -> Option<Position> {
        self.line_index.position(self.as_str(), offset)
    }

    fn range(&self, Span(start, end): Span) -> Option<Range> {
        Some(Range {
            start: self.position(start)?,
            end: self.position(end)?,
        })
    }

    fn lalrpop_offset(&self, line: usize, byte_column: usize) -> Option<usize> {
        // LALRPOP's FileText treats only `\n` as a line separator and reports
        // byte columns. Resolve that coordinate system before converting the
        // resulting absolute offset through the LSP line index.
        let line_start = if line == 0 {
            0
        } else {
            self.as_str()
                .match_indices('\n')
                .nth(line - 1)
                .map(|(offset, _)| offset + 1)?
        };
        let line_end = self
            .as_str()
            .get(line_start..)?
            .find('\n')
            .map_or(self.as_str().len(), |offset| line_start + offset);
        let offset = line_start.checked_add(byte_column)?.min(line_end);
        self.as_str().is_char_boundary(offset).then_some(offset)
    }

    fn diagnostic(&self, error: DiagnosticError) -> Diagnostic {
        let range = self.error_range(&error.loc).unwrap_or_default();
        Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("lalrpop".to_owned()),
            message: error.message,
            ..Default::default()
        }
    }

    fn error_range(&self, location: &ErrorLoc) -> Option<Range> {
        let (start, end) = match *location {
            ErrorLoc::Point(line, byte_column) => {
                let start = self.lalrpop_offset(line, byte_column)?;
                let end = self
                    .as_str()
                    .get(start..)?
                    .chars()
                    .next()
                    .map_or(start, |character| start + character.len_utf8());
                (start, end)
            }
            ErrorLoc::Span { lo, hi } => (
                self.lalrpop_offset(lo.0, lo.1)?,
                self.lalrpop_offset(hi.0, hi.1)?,
            ),
        };

        self.range(Span(start, end))
    }
}

struct AnalysisSnapshot {
    source: SourceText,
    file: LalrpopFile,
}

enum AnalysisUpdate {
    Valid(Box<LalrpopFile>),
    Invalid(Box<Diagnostic>),
}

pub(crate) struct DocumentUpdate {
    version: i32,
    source: SourceText,
    analysis: AnalysisUpdate,
}

impl DocumentUpdate {
    pub(crate) fn new(version: i32, text: String) -> Self {
        let source = SourceText::new(text);
        let analysis = match LalrpopFile::new(source.as_str()) {
            Ok(file) => AnalysisUpdate::Valid(Box::new(file)),
            Err(error) => AnalysisUpdate::Invalid(Box::new(source.diagnostic(error))),
        };

        Self {
            version,
            source,
            analysis,
        }
    }

    pub(crate) fn version(&self) -> i32 {
        self.version
    }
}

/// Current document text plus the latest valid semantic analysis.
///
/// When an edit is temporarily invalid, the previous analysis is retained and
/// safely mapped onto unchanged prefix and suffix regions of the current text.
pub(crate) struct ParsedDocument {
    version: i32,
    source: SourceText,
    analysis: Option<AnalysisSnapshot>,
    offset_map: Option<OffsetMap>,
}

impl ParsedDocument {
    pub(crate) fn new(update: DocumentUpdate) -> (Self, Vec<Diagnostic>) {
        let mut document = Self {
            version: update.version,
            source: update.source.clone(),
            analysis: None,
            offset_map: None,
        };
        let diagnostics = document.install_analysis(update.analysis);
        (document, diagnostics)
    }

    /// Applies a newer update and returns its diagnostics. Stale updates are
    /// ignored and return `None`.
    pub(crate) fn apply(&mut self, update: DocumentUpdate) -> Option<Vec<Diagnostic>> {
        if update.version <= self.version {
            return None;
        }

        self.version = update.version;
        self.source = update.source;
        Some(self.install_analysis(update.analysis))
    }

    pub(crate) fn version(&self) -> i32 {
        self.version
    }

    pub(crate) fn text(&self) -> &str {
        self.source.as_str()
    }

    pub(crate) fn range(&self, span: Span) -> Option<Range> {
        self.source.range(span)
    }

    pub(crate) fn line_index(&self) -> &LineIndex {
        &self.source.line_index
    }

    pub(crate) fn symbol_at(&self, position: Position) -> Option<SymbolHit> {
        let analysis = self.analysis.as_ref()?;
        let current_offset = self.source.offset(position)?;
        let analysis_offset = self.offset_map?.current_to_analysis(current_offset)?;
        let (analysis_span, item) =
            LalrpopFile::closest_hit(analysis.file.hit_offset_in_spans(analysis_offset))?;
        let span = self.map_analysis_span(analysis_span)?;
        Some(SymbolHit { span, item })
    }

    pub(crate) fn definition_span(&self, name: &str) -> Option<Span> {
        let span = *self.analysis.as_ref()?.file.definitions.get(name)?;
        self.map_analysis_span(span)
    }

    pub(crate) fn reference_spans(&self, name: &str) -> Vec<Span> {
        self.analysis
            .as_ref()
            .and_then(|analysis| analysis.file.references.get(name))
            .into_iter()
            .flatten()
            .filter_map(|span| self.map_analysis_span(*span))
            .collect()
    }

    pub(crate) fn occurrences(&self, name: &str) -> Vec<Occurrence> {
        let mut occurrences = Vec::new();
        if let Some(span) = self.definition_span(name) {
            occurrences.push(Occurrence {
                span,
                kind: OccurrenceKind::Definition,
            });
        }
        occurrences.extend(
            self.reference_spans(name)
                .into_iter()
                .map(|span| Occurrence {
                    span,
                    kind: OccurrenceKind::Reference,
                }),
        );
        occurrences.sort_unstable_by_key(|occurrence| (occurrence.span.0, occurrence.span.1));
        occurrences
    }

    pub(crate) fn definitions(&self) -> Vec<(String, Span)> {
        let mut definitions: Vec<_> = self
            .analysis
            .as_ref()
            .into_iter()
            .flat_map(|analysis| &analysis.file.definitions)
            .filter_map(|(name, span)| Some((name.clone(), self.map_analysis_span(*span)?)))
            .collect();
        definitions.sort_unstable_by_key(|(_, span)| (span.0, span.1));
        definitions
    }

    pub(crate) fn definition_source(&self, name: &str) -> Option<&str> {
        let analysis = self.analysis.as_ref()?;
        let span = analysis.file.definitions.get(name)?;
        analysis.source.as_str().get(span.0..span.1)
    }

    pub(crate) fn type_decl(&self, name: &str) -> Option<&TypeDecl> {
        self.analysis.as_ref()?.file.definition_type_decls.get(name)
    }

    pub(crate) fn analysis_file(&self) -> Option<&LalrpopFile> {
        Some(&self.analysis.as_ref()?.file)
    }

    pub(crate) fn map_analysis_span(&self, span: Span) -> Option<Span> {
        self.offset_map?.analysis_span_to_current(span)
    }

    fn install_analysis(&mut self, update: AnalysisUpdate) -> Vec<Diagnostic> {
        match update {
            AnalysisUpdate::Valid(file) => {
                self.analysis = Some(AnalysisSnapshot {
                    source: self.source.clone(),
                    file: *file,
                });
                self.offset_map = Some(OffsetMap::between(self.text(), self.text()));
                Vec::new()
            }
            AnalysisUpdate::Invalid(diagnostic) => {
                self.offset_map = self.analysis.as_ref().map(|analysis| {
                    OffsetMap::between(analysis.source.as_str(), self.source.as_str())
                });
                vec![*diagnostic]
            }
        }
    }
}

pub(crate) struct SymbolHit {
    pub(crate) span: Span,
    pub(crate) item: SpanItem,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OccurrenceKind {
    Definition,
    Reference,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Occurrence {
    pub(crate) span: Span,
    pub(crate) kind: OccurrenceKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OffsetMap {
    common_prefix: usize,
    analysis_suffix_start: usize,
    current_suffix_start: usize,
}

impl OffsetMap {
    fn between(analysis: &str, current: &str) -> Self {
        let mut common_prefix = analysis
            .as_bytes()
            .iter()
            .zip(current.as_bytes())
            .take_while(|(left, right)| left == right)
            .count();
        while !analysis.is_char_boundary(common_prefix) || !current.is_char_boundary(common_prefix)
        {
            common_prefix -= 1;
        }

        let max_suffix = (analysis.len() - common_prefix).min(current.len() - common_prefix);
        let mut common_suffix = analysis
            .as_bytes()
            .iter()
            .rev()
            .zip(current.as_bytes().iter().rev())
            .take(max_suffix)
            .take_while(|(left, right)| left == right)
            .count();
        while !analysis.is_char_boundary(analysis.len() - common_suffix)
            || !current.is_char_boundary(current.len() - common_suffix)
        {
            common_suffix -= 1;
        }

        Self {
            common_prefix,
            analysis_suffix_start: analysis.len() - common_suffix,
            current_suffix_start: current.len() - common_suffix,
        }
    }

    fn current_to_analysis(self, offset: usize) -> Option<usize> {
        if offset >= self.current_suffix_start {
            return Some(self.analysis_suffix_start + offset - self.current_suffix_start);
        }
        (offset <= self.common_prefix).then_some(offset)
    }

    fn analysis_span_to_current(self, Span(start, end): Span) -> Option<Span> {
        if end <= self.common_prefix {
            return Some(Span(start, end));
        }
        if start >= self.analysis_suffix_start {
            let shift = |offset| self.current_suffix_start + offset - self.analysis_suffix_start;
            return Some(Span(shift(start), shift(end)));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_document() -> ParsedDocument {
        let text = "grammar;\nStart: () = { Atom };\nAtom: () = { => () };\n";
        ParsedDocument::new(DocumentUpdate::new(1, text.to_owned())).0
    }

    #[test]
    fn rejects_stale_document_versions() {
        let mut document = valid_document();
        let stale = DocumentUpdate::new(1, "grammar;".to_owned());

        assert_eq!(document.apply(stale), None);
        assert_eq!(document.version(), 1);
        assert!(document.definition_span("Atom").is_some());
    }

    #[test]
    fn retains_analysis_for_unchanged_regions_after_parse_errors() {
        let mut document = valid_document();
        let invalid = "grammar;\n?\nStart: () = { Atom };\nAtom: () = { => () };\n";
        let diagnostics = document
            .apply(DocumentUpdate::new(2, invalid.to_owned()))
            .expect("new update");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(document.version(), 2);
        assert_eq!(document.definition_span("Atom"), Some(Span(33, 37)));
        let hit = document
            .symbol_at(Position::new(2, 16))
            .expect("unchanged reference should remain available");
        assert!(matches!(hit.item, SpanItem::Reference(ref name) if name == "Atom"));
    }

    #[test]
    fn hides_analysis_spans_overlapping_an_invalid_edit() {
        let mut document = valid_document();
        let invalid = "grammar;\nSta?rt: () = { Atom };\nAtom: () = { => () };\n";
        document
            .apply(DocumentUpdate::new(2, invalid.to_owned()))
            .expect("new update");

        assert_eq!(document.definition_span("Start"), None);
        assert!(document.definition_span("Atom").is_some());
    }

    #[test]
    fn diagnostics_use_utf16_positions() {
        let source = SourceText::new("😀?".to_owned());
        let range = source
            .error_range(&ErrorLoc::Point(0, 4))
            .expect("valid diagnostic range");

        assert_eq!(range, Range::new(Position::new(0, 2), Position::new(0, 3)));
    }

    #[test]
    fn diagnostics_translate_lalrpop_coordinates_before_lsp_line_endings() {
        let source = SourceText::new("a\rb?".to_owned());
        let range = source
            .error_range(&ErrorLoc::Point(0, 3))
            .expect("valid diagnostic range");

        assert_eq!(range, Range::new(Position::new(1, 1), Position::new(1, 2)));
    }

    #[test]
    fn offset_map_only_maps_unchanged_prefixes_and_suffixes() {
        let map = OffsetMap::between("abc middle xyz", "abc changed xyz");

        assert_eq!(map.current_to_analysis(2), Some(2));
        assert_eq!(map.current_to_analysis(6), None);
        assert_eq!(map.current_to_analysis(12), Some(11));
        assert_eq!(map.analysis_span_to_current(Span(0, 3)), Some(Span(0, 3)));
        assert_eq!(map.analysis_span_to_current(Span(4, 10)), None);
        assert_eq!(
            map.analysis_span_to_current(Span(11, 14)),
            Some(Span(12, 15))
        );
    }
}
