use lalrpop::lsp::{LalrpopFile, Span, SpanItem};
use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokensLegend,
};

use crate::line_index::LineIndex;

const FUNCTION_TOKEN: u32 = 0;
const MACRO_TOKEN: u32 = 1;
const DECLARATION_MODIFIER: u32 = 1 << 0;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct AbsoluteToken {
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
    token_modifiers_bitset: u32,
}

pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![SemanticTokenType::FUNCTION, SemanticTokenType::MACRO],
        token_modifiers: vec![SemanticTokenModifier::DECLARATION],
    }
}

pub fn full_mapped(
    text: &str,
    line_index: &LineIndex,
    file: &LalrpopFile,
    mut map_span: impl FnMut(Span) -> Option<Span>,
) -> Vec<SemanticToken> {
    let mut tokens: Vec<_> = file
        .span_items
        .iter()
        .filter_map(|(span, item)| absolute_token(text, line_index, file, map_span(*span)?, item))
        .collect();
    tokens.sort_unstable();
    tokens.dedup();
    encode_relative(tokens)
}

fn absolute_token(
    text: &str,
    line_index: &LineIndex,
    file: &LalrpopFile,
    span: Span,
    item: &SpanItem,
) -> Option<AbsoluteToken> {
    let (name, token_modifiers_bitset) = match item {
        SpanItem::Grammar => return None,
        SpanItem::Definition(name) => (name, DECLARATION_MODIFIER),
        SpanItem::Reference(name) => (name, 0),
    };

    // Only classify names resolved by the LALRPOP model. Macro parameters are
    // represented as references too, but are scoped locally and do not appear
    // in the global definition table.
    let declaration = file.definition_type_decls.get(name)?;
    let token_type = if declaration.args.is_empty() {
        FUNCTION_TOKEN
    } else {
        MACRO_TOKEN
    };

    let start_offset = name_offset(text, span, name)?;
    let end_offset = start_offset.checked_add(name.len())?;
    let token_text = text.get(start_offset..end_offset)?;
    let position = line_index.position(text, start_offset)?;

    Some(AbsoluteToken {
        line: position.line,
        start: position.character,
        length: u32::try_from(token_text.encode_utf16().count()).ok()?,
        token_type,
        token_modifiers_bitset,
    })
}

fn name_offset(text: &str, span: Span, name: &str) -> Option<usize> {
    let span_text = text.get(span.0..span.1)?;
    if span_text.starts_with(name) {
        return Some(span.0);
    }

    // Escaped nonterminal definitions include the surrounding backticks in
    // their span, while LALRPOP exposes the unescaped name.
    span_text
        .strip_prefix('`')?
        .starts_with(name)
        .then_some(span.0 + 1)
}

fn encode_relative(tokens: Vec<AbsoluteToken>) -> Vec<SemanticToken> {
    let mut previous_line = 0;
    let mut previous_start = 0;

    tokens
        .into_iter()
        .map(|token| {
            let delta_line = token.line - previous_line;
            let delta_start = if delta_line == 0 {
                token.start - previous_start
            } else {
                token.start
            };
            previous_line = token.line;
            previous_start = token.start;

            SemanticToken {
                delta_line,
                delta_start,
                length: token.length,
                token_type: token.token_type,
                token_modifiers_bitset: token.token_modifiers_bitset,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> LalrpopFile {
        match LalrpopFile::new(text) {
            Ok(file) => file,
            Err(_) => panic!("test grammar should parse"),
        }
    }

    fn full(text: &str, file: &LalrpopFile) -> Vec<SemanticToken> {
        full_mapped(text, &LineIndex::new(text), file, Some)
    }

    fn decode(tokens: &[SemanticToken]) -> Vec<(u32, u32, u32, u32, u32)> {
        let mut line = 0;
        let mut start = 0;
        tokens
            .iter()
            .map(|token| {
                line += token.delta_line;
                start = if token.delta_line == 0 {
                    start + token.delta_start
                } else {
                    token.delta_start
                };
                (
                    line,
                    start,
                    token.length,
                    token.token_type,
                    token.token_modifiers_bitset,
                )
            })
            .collect()
    }

    #[test]
    fn classifies_global_rules_and_macros() {
        let text =
            "grammar;\nStart: () = { Wrap<Atom> };\nWrap<T>: T = { T };\nAtom: () = { => () };\n";
        let file = parse(text);

        assert_eq!(
            decode(&full(text, &file)),
            vec![
                (1, 0, 5, FUNCTION_TOKEN, DECLARATION_MODIFIER),
                (1, 14, 4, MACRO_TOKEN, 0),
                (1, 19, 4, FUNCTION_TOKEN, 0),
                (2, 0, 4, MACRO_TOKEN, DECLARATION_MODIFIER),
                (3, 0, 4, FUNCTION_TOKEN, DECLARATION_MODIFIER),
            ]
        );
    }

    #[test]
    fn encodes_utf16_columns() {
        let text = "grammar;\nStart: () = { \"é\" Atom => () };\nAtom: () = { => () };\n";
        let file = parse(text);

        assert_eq!(
            decode(&full(text, &file)),
            vec![
                (1, 0, 5, FUNCTION_TOKEN, DECLARATION_MODIFIER),
                (1, 18, 4, FUNCTION_TOKEN, 0),
                (2, 0, 4, FUNCTION_TOKEN, DECLARATION_MODIFIER),
            ]
        );
    }

    #[test]
    fn mapped_tokens_use_current_text_positions() {
        let analysis_text = "grammar;\nStart: () = { Atom };\nAtom: () = { => () };\n";
        let current_text = "?\n\ngrammar;\nStart: () = { Atom };\nAtom: () = { => () };\n";
        let file = parse(analysis_text);
        let line_index = LineIndex::new(current_text);

        assert_eq!(
            decode(&full_mapped(
                current_text,
                &line_index,
                &file,
                |Span(start, end)| Some(Span(start + 3, end + 3)),
            )),
            vec![
                (3, 0, 5, FUNCTION_TOKEN, DECLARATION_MODIFIER),
                (3, 14, 4, FUNCTION_TOKEN, 0),
                (4, 0, 4, FUNCTION_TOKEN, DECLARATION_MODIFIER),
            ]
        );
    }
}
