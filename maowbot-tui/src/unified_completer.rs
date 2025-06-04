// Unified completer that uses the common-ui completion engine
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::validate::{Validator, ValidationContext, ValidationResult};
use rustyline::{Helper, Context};
use rustyline_derive::Helper;
use std::borrow::Cow;
use std::sync::Arc;

use maowbot_common_ui::completion::{
    CompletionEngine, CompletionContext, CompletionScope, 
    CompletionEngineBuilder, CompletionConfig,
    providers::{
        TuiCommandCompletionProvider, CommandCompletionProvider,
        EmoteCompletionProvider, UserCompletionProvider
    }
};
use maowbot_common_ui::GrpcClient;

#[derive(Helper)]
pub struct UnifiedCompleter {
    engine: Arc<CompletionEngine>,
    runtime_handle: tokio::runtime::Handle,
    highlighter: MatchingBracketHighlighter,
    hinter: HistoryHinter,
}

impl UnifiedCompleter {
    pub fn new(client: Arc<GrpcClient>) -> Self {
        // Build the completion engine with all providers
        let engine = CompletionEngineBuilder::new()
            .with_config(CompletionConfig {
                max_items: 20,
                fuzzy_matching: true,
                min_prefix_length: 1,
                show_descriptions: true,
                show_icons: true,
                group_by_category: true,
                case_sensitive: false,
            })
            .with_provider(Box::new(TuiCommandCompletionProvider::new()))
            .with_provider(Box::new(UserCompletionProvider::new(client.clone())))
            .with_provider(Box::new(CommandCompletionProvider::new(client.clone())))
            .with_provider(Box::new(EmoteCompletionProvider::new(client.clone())))
            .build();
        
        Self {
            engine: Arc::new(engine),
            runtime_handle: tokio::runtime::Handle::current(),
            highlighter: MatchingBracketHighlighter::new(),
            hinter: HistoryHinter::new(),
        }
    }
}

impl Completer for UnifiedCompleter {
    type Candidate = Pair;
    
    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Result<(usize, Vec<Pair>), ReadlineError> {
        let engine = self.engine.clone();
        
        // Create completion context
        let context = CompletionContext::new(
            CompletionScope::TuiCommand,
            line.to_string(),
            pos,
        );
        
        // Get completions from the engine
        let items = self.runtime_handle.block_on(async move {
            engine.get_completions(&context).await
        });
        
        // Convert to rustyline pairs
        let candidates: Vec<Pair> = items
            .into_iter()
            .map(|item| {
                let display = if let Some(desc) = item.description {
                    format!(
                        "{}{:<20} {}",
                        if item.icon.is_some() { format!("{} ", item.icon.unwrap()) } else { String::new() },
                        item.display,
                        desc
                    )
                } else {
                    format!(
                        "{}{}",
                        if item.icon.is_some() { format!("{} ", item.icon.unwrap()) } else { String::new() },
                        item.display
                    )
                };
                
                Pair {
                    display,
                    replacement: item.replacement,
                }
            })
            .collect();
        
        // Calculate start position
        let start = if line[..pos].ends_with(' ') {
            pos
        } else {
            line[..pos].rfind(' ').map(|i| i + 1).unwrap_or(0)
        };
        
        Ok((start, candidates))
    }
}

impl Validator for UnifiedCompleter {
    fn validate(&self, _ctx: &mut ValidationContext) -> Result<ValidationResult, ReadlineError> {
        Ok(ValidationResult::Valid(None))
    }
}

impl Hinter for UnifiedCompleter {
    type Hint = String;
    
    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

impl Highlighter for UnifiedCompleter {
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }
    
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        self.highlighter.highlight_prompt(prompt, default)
    }
    
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(format!("\x1b[2m{}\x1b[0m", hint))
    }
    
    fn highlight_char(&self, line: &str, pos: usize, forced: bool) -> bool {
        self.highlighter.highlight_char(line, pos, forced)
    }
}