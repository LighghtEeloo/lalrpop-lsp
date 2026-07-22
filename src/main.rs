mod semantic_tokens;

use lalrpop::lsp::{DiagnosticError, LalrpopFile, SpanItem, TypeDecl};
use tower_lsp::{LspService, Server};

use dashmap::DashMap;
use tower_lsp::lsp_types::*;
use tower_lsp::{jsonrpc::Result, Client, LanguageServer};

/// Text document item for file changes.
pub struct TextDocumentSyncItem {
    /// URI of the document.
    pub uri: Url,
    /// Text of the document.
    pub text: String,
    /// Version of the document.
    pub version: i32,
}

/// LALRPOP Language Server Protocol
pub struct LalrpopLsp {
    client: Client,
    files: DashMap<String, ParsedDocument>,
}

struct ParsedDocument {
    text: String,
    file: LalrpopFile,
}

fn hover_signature(name: &str, type_decl: &str) -> MarkupContent {
    MarkupContent {
        kind: MarkupKind::Markdown,
        value: format!("```LALRPOP\n{name}{type_decl}\n```"),
    }
}

impl LalrpopLsp {
    /// Create a new LALRPOP Language Server Protocol
    pub fn new(client: Client) -> Self {
        Self {
            client,
            files: DashMap::new(),
        }
    }
    /// Get the grammar for a given URI
    pub async fn on_change(&self, params: TextDocumentSyncItem) {
        self.client
            .log_message(
                MessageType::INFO,
                format!("on change: {}", params.uri.as_str()),
                // format!("on change:\n{}", params.text.as_str()),
            )
            .await;

        let uri = params.uri.to_string();
        let file = match LalrpopFile::new(params.text.as_str()) {
            Ok(file) => file,
            Err(DiagnosticError { loc, message }) => {
                self.files.remove(&uri);
                let range = {
                    let (lo, hi) = match loc {
                        lalrpop::lsp::ErrorLoc::Point(line, col) => ((line, col), (line, col + 1)),
                        lalrpop::lsp::ErrorLoc::Span { lo, hi } => (lo, hi),
                    };
                    let start = Position {
                        line: lo.0 as u32,
                        character: lo.1 as u32,
                    };
                    let end = Position {
                        line: hi.0 as u32,
                        character: hi.1 as u32,
                    };
                    Range { start, end }
                };
                // self.client
                //     .log_message(MessageType::ERROR, format!("error: {}", err.message))
                //     .await;
                self.client
                    .publish_diagnostics(
                        params.uri,
                        vec![Diagnostic {
                            range,
                            severity: None,
                            code: None,
                            code_description: None,
                            source: Some("lalrpop".to_string()),
                            message,
                            related_information: None,
                            tags: None,
                            data: None,
                        }],
                        Some(params.version),
                    )
                    .await;
                return;
            }
        };

        // self.client
        //     .log_message(MessageType::INFO, format!("parsed:\n{:#?}", file.tree))
        //     .await;
        self.client
            .log_message(
                MessageType::INFO,
                format!("spanned:\n{:#?}", file.span_items),
            )
            .await;
        // self.client
        //     .log_message(MessageType::INFO, format!("normalized:\n{:#?}", file.repr))
        //     .await;

        // update
        self.files.insert(
            uri.clone(),
            ParsedDocument {
                text: params.text,
                file,
            },
        );
        // refresh diagnostics
        self.client
            .publish_diagnostics(params.uri, vec![], Some(params.version))
            .await;
    }

    /// A helper function to convert an offset to a position.
    fn offset_to_position(document: &ParsedDocument, offset: usize) -> Position {
        let (line, col) = document.file.line_col(offset);
        Position {
            line: line as u32,
            character: col as u32,
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for LalrpopLsp {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                document_symbol_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: semantic_tokens::legend(),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            ..Default::default()
                        },
                    ),
                ),
                ..Default::default()
            },
            ..Default::default()
        })
    }
    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "initialized!")
            .await;
    }
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file opened!")
            .await;
        self.on_change(TextDocumentSyncItem {
            uri: params.text_document.uri,
            text: params.text_document.text,
            version: params.text_document.version,
        })
        .await
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        self.on_change(TextDocumentSyncItem {
            uri: params.text_document.uri,
            text: std::mem::take(&mut params.content_changes[0].text),
            version: params.text_document.version,
        })
        .await
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file saved!")
            .await;
    }
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.files.remove(params.text_document.uri.as_str());
        self.client
            .log_message(MessageType::INFO, "file closed!")
            .await;
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let Some(document) = self.files.get(uri.as_str()) else {
            return Ok(None);
        };
        let position = params.text_document_position_params.position;
        let Some(offset) = document
            .file
            .offset_from_line_col(position.line as usize, position.character as usize)
        else {
            return Ok(None);
        };
        let hits = document.file.hit_offset_in_spans(offset);
        // self.client
        //     .log_message(
        //         MessageType::INFO,
        //         format!("goto definition hits: {:#?}", hits),
        //     )
        //     .await;
        let Some((_span, span_item)) = LalrpopFile::closest_hit(hits) else {
            return Ok(None);
        };
        match span_item {
            SpanItem::Grammar => {}
            SpanItem::Definition(def) => {
                // Todo: actually we return the references here
                let Some(spans) = document.file.references.get(&def) else {
                    return Ok(None);
                };
                return Ok(Some(GotoDefinitionResponse::Array(
                    spans
                        .iter()
                        .map(|span| {
                            let start = Self::offset_to_position(document.value(), span.0);
                            let end = Self::offset_to_position(document.value(), span.1);
                            Location {
                                uri: uri.to_owned(),
                                range: Range { start, end },
                            }
                        })
                        .collect(),
                )));
            }
            SpanItem::Reference(def) => {
                let Some(span) = document.file.definitions.get(&def) else {
                    return Ok(None);
                };
                let start = Self::offset_to_position(document.value(), span.0);
                let end = Self::offset_to_position(document.value(), span.1);
                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri,
                    range: Range { start, end },
                })));
            }
        }
        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let Some(document) = self.files.get(uri.as_str()) else {
            return Ok(None);
        };
        let position = params.text_document_position.position;
        let Some(offset) = document
            .file
            .offset_from_line_col(position.line as usize, position.character as usize)
        else {
            return Ok(None);
        };
        let hits = document.file.hit_offset_in_spans(offset);
        // self.client
        //     .log_message(
        //         MessageType::INFO,
        //         format!("references hits: {:#?}", hits),
        //     )
        //     .await;
        let Some((_span, span_item)) = LalrpopFile::closest_hit(hits) else {
            return Ok(None);
        };
        match span_item {
            SpanItem::Grammar => {}
            SpanItem::Reference(_) => {}
            SpanItem::Definition(def) => {
                let Some(spans) = document.file.references.get(&def) else {
                    return Ok(None);
                };
                return Ok(Some(
                    spans
                        .iter()
                        .map(|span| {
                            let start = Self::offset_to_position(document.value(), span.0);
                            let end = Self::offset_to_position(document.value(), span.1);
                            Location {
                                uri: uri.to_owned(),
                                range: Range { start, end },
                            }
                        })
                        .collect(),
                ));
            }
        }
        Ok(None)
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let Some(document) = self.files.get(uri.as_str()) else {
            return Ok(None);
        };
        let position = params.text_document_position_params.position;
        let Some(offset) = document
            .file
            .offset_from_line_col(position.line as usize, position.character as usize)
        else {
            return Ok(None);
        };
        let hits = document.file.hit_offset_in_spans(offset);
        // self.client
        //     .log_message(
        //         MessageType::INFO,
        //         format!("hover hits: {:#?}", hits),
        //     )
        //     .await;
        let Some((span, span_item)) = LalrpopFile::closest_hit(hits) else {
            return Ok(None);
        };
        match span_item {
            SpanItem::Grammar => {}
            SpanItem::Definition(def) | SpanItem::Reference(def) => {
                let Some(TypeDecl { args, ret }) = document.file.definition_type_decls.get(&def)
                else {
                    return Ok(None);
                };
                let definition_name = document
                    .file
                    .definitions
                    .get(&def)
                    .and_then(|span| document.text.get(span.0..span.1))
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        format!(
                            "{}{}",
                            def,
                            if args.is_empty() {
                                String::new()
                            } else {
                                format!("<{}>", args.join(", "))
                            }
                        )
                    });
                let type_decl = ret.as_ref().map_or(String::new(), |ty| format!(": {ty}"));
                let contents = HoverContents::Markup(hover_signature(&definition_name, &type_decl));
                let range = {
                    let start = Self::offset_to_position(document.value(), span.0);
                    let end = Self::offset_to_position(document.value(), span.1);
                    Range { start, end }
                };
                return Ok(Some(Hover {
                    contents,
                    range: Some(range),
                }));
            }
        }
        Ok(None)
    }
    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let Some(document) = self.files.get(uri.as_str()) else {
            return Ok(None);
        };
        let mut symbols = vec![];
        for (def, span) in &document.file.definitions {
            let range = {
                let start = Self::offset_to_position(document.value(), span.0);
                let end = Self::offset_to_position(document.value(), span.1);
                Range { start, end }
            };
            #[allow(deprecated)]
            symbols.push(DocumentSymbol {
                name: def.to_owned(),
                detail: None,
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                range,
                selection_range: range,
                children: None,
            });
        }
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let data = self
            .files
            .get(params.text_document.uri.as_str())
            .map(|document| semantic_tokens::full(&document.text, &document.file))
            .unwrap_or_default();

        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data,
        })))
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let (service, socket) = LspService::build(LalrpopLsp::new).finish();
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    Server::new(stdin, stdout, socket).serve(service).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_signatures_are_lalrpop_fragments() {
        let markup = hover_signature("String", ": String");

        assert_eq!(markup.kind, MarkupKind::Markdown);
        assert_eq!(markup.value, "```LALRPOP\nString: String\n```");
    }
}
