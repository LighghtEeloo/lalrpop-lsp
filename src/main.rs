mod document;
mod line_index;
mod semantic_tokens;

use document::{DocumentUpdate, OccurrenceKind, ParsedDocument};
use lalrpop::lsp::{Span, SpanItem, TypeDecl};
use tower_lsp::{LspService, Server};

use dashmap::{mapref::entry::Entry, DashMap};
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
        let uri = params.uri.to_string();
        if self
            .files
            .get(&uri)
            .is_some_and(|document| params.version <= document.version())
        {
            return;
        }

        let update = DocumentUpdate::new(params.version, params.text);
        let version = update.version();
        let diagnostics = match self.files.entry(uri) {
            Entry::Occupied(mut entry) => entry.get_mut().apply(update),
            Entry::Vacant(entry) => {
                let (document, diagnostics) = ParsedDocument::new(update);
                entry.insert(document);
                Some(diagnostics)
            }
        };
        if let Some(diagnostics) = diagnostics {
            self.client
                .publish_diagnostics(params.uri, diagnostics, Some(version))
                .await;
        }
    }

    fn location(document: &ParsedDocument, uri: Url, span: Span) -> Option<Location> {
        Some(Location {
            uri,
            range: document.range(span)?,
        })
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for LalrpopLsp {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                position_encoding: Some(PositionEncodingKind::UTF16),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_highlight_provider: Some(OneOf::Left(true)),
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

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let Some(change) = params.content_changes.into_iter().last() else {
            return;
        };
        self.on_change(TextDocumentSyncItem {
            uri: params.text_document.uri,
            text: change.text,
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
        let Some(hit) = document.symbol_at(position) else {
            return Ok(None);
        };
        let name = match hit.item {
            SpanItem::Definition(name) | SpanItem::Reference(name) => name,
            SpanItem::Grammar => return Ok(None),
        };
        let Some(span) = document.definition_span(&name) else {
            return Ok(None);
        };
        let Some(location) = Self::location(document.value(), uri, span) else {
            return Ok(None);
        };

        // Returning the declaration itself when invoked on a declaration lets
        // clients such as VS Code choose their usual "show references" fallback.
        Ok(Some(GotoDefinitionResponse::Scalar(location)))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let Some(document) = self.files.get(uri.as_str()) else {
            return Ok(None);
        };
        let position = params.text_document_position.position;
        let Some(hit) = document.symbol_at(position) else {
            return Ok(None);
        };
        let name = match hit.item {
            SpanItem::Definition(name) | SpanItem::Reference(name) => name,
            SpanItem::Grammar => return Ok(None),
        };

        let mut spans = document.reference_spans(&name);
        if params.context.include_declaration {
            if let Some(span) = document.definition_span(&name) {
                spans.push(span);
            }
        }
        spans.sort_unstable_by_key(|span| (span.0, span.1));

        Ok(Some(
            spans
                .into_iter()
                .filter_map(|span| Self::location(document.value(), uri.clone(), span))
                .collect(),
        ))
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let uri = params.text_document_position_params.text_document.uri;
        let Some(document) = self.files.get(uri.as_str()) else {
            return Ok(None);
        };
        let position = params.text_document_position_params.position;
        let Some(hit) = document.symbol_at(position) else {
            return Ok(None);
        };
        let name = match hit.item {
            SpanItem::Definition(name) | SpanItem::Reference(name) => name,
            SpanItem::Grammar => return Ok(None),
        };

        Ok(Some(
            document
                .occurrences(&name)
                .into_iter()
                .filter_map(|occurrence| {
                    Some(DocumentHighlight {
                        range: document.range(occurrence.span)?,
                        kind: Some(match occurrence.kind {
                            OccurrenceKind::Definition => DocumentHighlightKind::WRITE,
                            OccurrenceKind::Reference => DocumentHighlightKind::READ,
                        }),
                    })
                })
                .collect(),
        ))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let Some(document) = self.files.get(uri.as_str()) else {
            return Ok(None);
        };
        let position = params.text_document_position_params.position;
        let Some(hit) = document.symbol_at(position) else {
            return Ok(None);
        };
        match hit.item {
            SpanItem::Grammar => {}
            SpanItem::Definition(def) | SpanItem::Reference(def) => {
                let Some(TypeDecl { args, ret }) = document.type_decl(&def) else {
                    return Ok(None);
                };
                let definition_name = document
                    .definition_source(&def)
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
                let Some(range) = document.range(hit.span) else {
                    return Ok(None);
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
        for (def, span) in document.definitions() {
            let Some(range) = document.range(span) else {
                continue;
            };
            #[allow(deprecated)]
            symbols.push(DocumentSymbol {
                name: def,
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
            .and_then(|document| {
                Some(semantic_tokens::full_mapped(
                    document.text(),
                    document.line_index(),
                    document.analysis_file()?,
                    |span| document.map_analysis_span(span),
                ))
            })
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
    use futures::StreamExt;
    use serde_json::{json, Value};
    use tower::{Service, ServiceExt};
    use tower_lsp::jsonrpc::{Request, Response};

    async fn send(service: &mut LspService<LalrpopLsp>, request: Request) -> Option<Response> {
        service
            .ready()
            .await
            .expect("service should be ready")
            .call(request)
            .await
            .expect("request should be handled")
    }

    async fn request(
        service: &mut LspService<LalrpopLsp>,
        id: i64,
        method: &'static str,
        params: Value,
    ) -> Value {
        let response = send(
            service,
            Request::build(method).params(params).id(id).finish(),
        )
        .await
        .expect("request should return a response");
        response.into_parts().1.expect("request should succeed")
    }

    async fn notify(service: &mut LspService<LalrpopLsp>, method: &'static str, params: Value) {
        let response = send(service, Request::build(method).params(params).finish()).await;
        assert_eq!(response, None);
    }

    fn location(uri: &str, start: (u32, u32), end: (u32, u32)) -> Value {
        json!({
            "uri": uri,
            "range": {
                "start": { "line": start.0, "character": start.1 },
                "end": { "line": end.0, "character": end.1 },
            }
        })
    }

    #[test]
    fn hover_signatures_are_lalrpop_fragments() {
        let markup = hover_signature("String", ": String");

        assert_eq!(markup.kind, MarkupKind::Markdown);
        assert_eq!(markup.value, "```LALRPOP\nString: String\n```");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn navigation_uses_utf16_and_standard_symbol_semantics() {
        let (mut service, mut socket) = LspService::new(LalrpopLsp::new);
        let socket_task = tokio::spawn(async move { while socket.next().await.is_some() {} });
        let initialize =
            request(&mut service, 1, "initialize", json!({ "capabilities": {} })).await;
        assert_eq!(
            initialize["capabilities"]["positionEncoding"],
            json!("utf-16")
        );
        assert_eq!(
            initialize["capabilities"]["documentHighlightProvider"],
            json!(true)
        );

        let uri = "file:///tmp/navigation.lalrpop";
        let text = "grammar;\nStart: () = { \"😀\" Atom };\nAtom: () = { => () };\n";
        notify(
            &mut service,
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "lalrpop",
                    "version": 1,
                    "text": text,
                }
            }),
        )
        .await;

        let definition_from_reference = request(
            &mut service,
            2,
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": 1, "character": 20 },
            }),
        )
        .await;
        assert_eq!(definition_from_reference, location(uri, (2, 0), (2, 4)));

        // A definition request on the declaration returns that declaration.
        // VS Code can use this self-target to open its references UI.
        let definition_from_declaration = request(
            &mut service,
            3,
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": 2, "character": 1 },
            }),
        )
        .await;
        assert_eq!(definition_from_declaration, location(uri, (2, 0), (2, 4)));

        let references = request(
            &mut service,
            4,
            "textDocument/references",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": 1, "character": 20 },
                "context": { "includeDeclaration": false },
            }),
        )
        .await;
        assert_eq!(references, json!([location(uri, (1, 19), (1, 23))]));

        let references_from_declaration = request(
            &mut service,
            9,
            "textDocument/references",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": 2, "character": 1 },
                "context": { "includeDeclaration": false },
            }),
        )
        .await;
        assert_eq!(
            references_from_declaration,
            json!([location(uri, (1, 19), (1, 23))])
        );

        let references_with_declaration = request(
            &mut service,
            5,
            "textDocument/references",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": 1, "character": 20 },
                "context": { "includeDeclaration": true },
            }),
        )
        .await;
        assert_eq!(
            references_with_declaration,
            json!([
                location(uri, (1, 19), (1, 23)),
                location(uri, (2, 0), (2, 4)),
            ])
        );

        let highlights = request(
            &mut service,
            6,
            "textDocument/documentHighlight",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": 1, "character": 20 },
            }),
        )
        .await;
        assert_eq!(
            highlights,
            json!([
                {
                    "range": location(uri, (1, 19), (1, 23))["range"],
                    "kind": 2,
                },
                {
                    "range": location(uri, (2, 0), (2, 4))["range"],
                    "kind": 3,
                },
            ])
        );

        let invalid_text = "grammar;\n?\nStart: () = { \"😀\" Atom };\nAtom: () = { => () };\n";
        notify(
            &mut service,
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": uri, "version": 2 },
                "contentChanges": [{ "text": invalid_text }],
            }),
        )
        .await;

        let retained_definition = request(
            &mut service,
            7,
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": 2, "character": 20 },
            }),
        )
        .await;
        assert_eq!(retained_definition, location(uri, (3, 0), (3, 4)));

        // An older change must not overwrite the version-two document.
        notify(
            &mut service,
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": uri, "version": 1 },
                "contentChanges": [{ "text": "grammar;\nOnly: () = { => () };\n" }],
            }),
        )
        .await;
        let after_stale_change = request(
            &mut service,
            8,
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": 2, "character": 20 },
            }),
        )
        .await;
        assert_eq!(after_stale_change, location(uri, (3, 0), (3, 4)));
        socket_task.abort();
    }
}
