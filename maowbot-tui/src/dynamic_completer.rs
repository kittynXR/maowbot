// Dynamic completion that can fetch data from gRPC
use crate::completion::TuiCompleter;
use maowbot_common_ui::GrpcClient;
use rustyline::completion::Pair;
use std::sync::{Arc, Mutex};
use tokio::runtime::Handle;

pub struct DynamicCompleter {
    base_completer: TuiCompleter,
    client: Arc<GrpcClient>,
    runtime_handle: Handle,
    cache: Arc<Mutex<CompletionCache>>,
}

#[derive(Default)]
struct CompletionCache {
    usernames: Vec<String>,
    platform_names: Vec<String>,
    plugin_names: Vec<String>,
    last_refresh: std::time::Instant,
}

impl DynamicCompleter {
    pub fn new(client: Arc<GrpcClient>) -> Self {
        Self {
            base_completer: TuiCompleter::new(),
            client,
            runtime_handle: Handle::current(),
            cache: Arc::new(Mutex::new(CompletionCache {
                last_refresh: std::time::Instant::now(),
                ..Default::default()
            })),
        }
    }
    
    pub fn complete_dynamic(&self, parts: &[&str]) -> Vec<Pair> {
        let mut candidates = Vec::new();
        
        match (parts.get(0), parts.get(1)) {
            // User commands that need username completion
            (Some(&"user"), Some(&"info" | &"edit" | &"remove" | &"note" | &"merge" | &"roles" | &"analysis")) => {
                if parts.len() == 3 {
                    let prefix = parts[2].to_lowercase();
                    candidates.extend(self.get_usernames()
                        .into_iter()
                        .filter(|name| name.to_lowercase().starts_with(&prefix))
                        .map(|name| Pair {
                            display: name.clone(),
                            replacement: name,
                        }));
                }
            }
            
            // Platform commands that need platform type completion
            (Some(&"platform"), Some(&"add")) => {
                if parts.len() == 3 {
                    let platforms = vec!["twitch", "discord", "vrchat"];
                    let prefix = parts[2];
                    candidates.extend(platforms.into_iter()
                        .filter(|p| p.starts_with(prefix))
                        .map(|p| Pair {
                            display: p.to_string(),
                            replacement: p.to_string(),
                        }));
                }
            }
            
            // Account commands that need platform names
            (Some(&"account"), Some(&"add" | &"list")) => {
                if parts.len() == 3 {
                    let prefix = parts[2];
                    candidates.extend(self.get_platform_names()
                        .into_iter()
                        .filter(|name| name.starts_with(prefix))
                        .map(|name| Pair {
                            display: name.clone(),
                            replacement: name,
                        }));
                }
            }
            
            // Plugin commands that need plugin names
            (Some(&"plugin"), Some(&"enable" | &"disable" | &"remove")) => {
                if parts.len() == 3 {
                    let prefix = parts[2].to_lowercase();
                    candidates.extend(self.get_plugin_names()
                        .into_iter()
                        .filter(|name| name.to_lowercase().starts_with(&prefix))
                        .map(|name| Pair {
                            display: name.clone(),
                            replacement: name,
                        }));
                }
            }
            
            // Connection commands
            (Some(&"connection"), Some(&"start" | &"stop")) => {
                if parts.len() == 3 {
                    let platforms = vec!["twitch-irc", "twitch-eventsub", "discord", "vrchat"];
                    let prefix = parts[2];
                    candidates.extend(platforms.into_iter()
                        .filter(|p| p.starts_with(prefix))
                        .map(|p| Pair {
                            display: p.to_string(),
                            replacement: p.to_string(),
                        }));
                }
            }
            
            // Credential commands
            (Some(&"credential"), Some(&"list" | &"batch-refresh")) => {
                if parts.len() == 3 {
                    let platforms = vec!["twitch", "discord", "vrchat"];
                    let prefix = parts[2];
                    candidates.extend(platforms.into_iter()
                        .filter(|p| p.starts_with(prefix))
                        .map(|p| Pair {
                            display: p.to_string(),
                            replacement: p.to_string(),
                        }));
                }
            }
            
            _ => {}
        }
        
        candidates
    }
    
    fn get_usernames(&self) -> Vec<String> {
        // Try to get from cache first
        {
            let cache = self.cache.lock().unwrap();
            if cache.last_refresh.elapsed() < std::time::Duration::from_secs(300) && !cache.usernames.is_empty() {
                return cache.usernames.clone();
            }
        }
        
        // Fetch from gRPC
        let client = self.client.clone();
        let cache = self.cache.clone();
        
        let usernames = self.runtime_handle.block_on(async {
            use maowbot_proto::maowbot::services::ListUsersRequest;
            use maowbot_proto::maowbot::common::PageRequest;
            
            let request = ListUsersRequest {
                page: Some(PageRequest {
                    page_size: 100,
                    page_token: String::new(),
                }),
                filter: None,
                order_by: String::new(),
                descending: false,
            };
            
            match client.user.clone().list_users(request).await {
                Ok(response) => {
                    response.into_inner().users
                        .into_iter()
                        .map(|u| u.global_username)
                        .collect()
                }
                Err(_) => vec![],
            }
        });
        
        // Update cache
        let mut cache = cache.lock().unwrap();
        cache.usernames = usernames.clone();
        cache.last_refresh = std::time::Instant::now();
        
        usernames
    }
    
    fn get_platform_names(&self) -> Vec<String> {
        {
            let cache = self.cache.lock().unwrap();
            if cache.last_refresh.elapsed() < std::time::Duration::from_secs(300) && !cache.platform_names.is_empty() {
                return cache.platform_names.clone();
            }
        }
        
        let client = self.client.clone();
        let cache = self.cache.clone();
        
        let platforms = self.runtime_handle.block_on(async {
            use maowbot_proto::maowbot::services::ListPlatformConfigsRequest;
            
            let request = ListPlatformConfigsRequest {
                platforms: vec![],
                page: None,
            };
            
            match client.platform.clone().list_platform_configs(request).await {
                Ok(response) => {
                    response.into_inner().configs
                        .into_iter()
                        .map(|c| format!("{:?}", c.platform).to_lowercase())
                        .collect()
                }
                Err(_) => vec!["twitch", "discord", "vrchat"].into_iter().map(String::from).collect(),
            }
        });
        
        let mut cache = cache.lock().unwrap();
        cache.platform_names = platforms.clone();
        
        platforms
    }
    
    fn get_plugin_names(&self) -> Vec<String> {
        {
            let cache = self.cache.lock().unwrap();
            if cache.last_refresh.elapsed() < std::time::Duration::from_secs(300) && !cache.plugin_names.is_empty() {
                return cache.plugin_names.clone();
            }
        }
        
        let client = self.client.clone();
        let cache = self.cache.clone();
        
        let plugins = self.runtime_handle.block_on(async {
            use maowbot_proto::maowbot::services::ListPluginsRequest;
            
            let request = ListPluginsRequest {
                active_only: false,
                include_system_plugins: true,
            };
            
            match client.plugin.clone().list_plugins(request).await {
                Ok(response) => {
                    response.into_inner().plugins
                        .into_iter()
                        .filter_map(|p| p.plugin.map(|plugin| plugin.plugin_name))
                        .collect()
                }
                Err(_) => vec![],
            }
        });
        
        let mut cache = cache.lock().unwrap();
        cache.plugin_names = plugins.clone();
        
        plugins
    }
}