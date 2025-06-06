#[cfg(test)]
mod tests {
    use maowbot_common_ui::completion::{
        CompletionContext, CompletionScope, 
        CompletionEngineBuilder, CompletionConfig,
        providers::TuiCommandCompletionProvider,
    };

    #[tokio::test]
    async fn test_osc_command_completion() {
        // Create a fresh engine for each test to avoid caching issues
        let mut engine_builder = CompletionEngineBuilder::new();
        engine_builder = engine_builder.with_config(CompletionConfig {
            max_items: 50,
            fuzzy_matching: false,  // Disable fuzzy matching for test
            min_prefix_length: 0,
            show_descriptions: true,
            show_icons: true,
            group_by_category: false,
            case_sensitive: false,
        });
        engine_builder = engine_builder.with_provider(Box::new(TuiCommandCompletionProvider::new()));
        let engine = engine_builder.build();
        
        // Test OSC appears in top-level commands
        let context = CompletionContext::new(
            CompletionScope::TuiCommand,
            "".to_string(),
            0,
        );
        
        let completions = engine.get_completions(&context).await;
        let has_osc = completions.iter().any(|c| c.display == "osc");
        assert!(has_osc, "OSC command not found in top-level completions");
        
        // Print all commands for debugging
        println!("\nAll top-level commands:");
        for c in &completions {
            println!("  {} - {:?}", c.display, c.description);
        }
        
        // Test OSC subcommands
        let context = CompletionContext::new(
            CompletionScope::TuiCommand,
            "osc ".to_string(),
            4,
        );
        
        // Debug: print context details
        println!("\nDebug context for 'osc ':");
        println!("  text_before_cursor: '{}'", context.text_before_cursor());
        println!("  current_word: '{}'", context.current_word());
        println!("  previous_words: {:?}", context.previous_words());
        
        let completions = engine.get_completions(&context).await;
        println!("  completions count: {}", completions.len());
        
        // More debug output
        if !completions.is_empty() {
            println!("  First few completions:");
            for (i, c) in completions.iter().take(5).enumerate() {
                println!("    {}: {} (category: {:?})", i, c.display, c.category);
            }
        }
        
        assert!(!completions.is_empty(), "No OSC subcommands found");
        
        let subcommands: Vec<_> = completions.iter().map(|c| c.display.as_str()).collect();
        println!("\nOSC subcommands found: {:?}", subcommands);
        
        assert!(subcommands.contains(&"start"), "Missing 'start' subcommand");
        assert!(subcommands.contains(&"toggle"), "Missing 'toggle' subcommand");
        assert!(subcommands.contains(&"chatbox"), "Missing 'chatbox' subcommand");
        assert!(subcommands.contains(&"set"), "Missing 'set' subcommand");
        assert!(subcommands.contains(&"raw"), "Missing 'raw' subcommand");
        
        // Test nested subcommands
        let context = CompletionContext::new(
            CompletionScope::TuiCommand,
            "osc toggle ".to_string(),
            11,
        );
        
        let completions = engine.get_completions(&context).await;
        let toggle_subs: Vec<_> = completions.iter().map(|c| c.display.as_str()).collect();
        println!("\nOSC toggle subcommands: {:?}", toggle_subs);
        
        assert!(toggle_subs.contains(&"list"), "Missing 'list' in toggle subcommands");
        assert!(toggle_subs.contains(&"test"), "Missing 'test' in toggle subcommands");
    }
}