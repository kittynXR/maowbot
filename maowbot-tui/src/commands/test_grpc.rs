// Simple test command to verify gRPC connectivity
use maowbot_common_ui::GrpcClient;

pub async fn handle_test_grpc_command(args: &[&str]) -> String {
    // Try to connect to the gRPC server
    let addr = if args.is_empty() {
        "https://127.0.0.1:9999" // Default address with HTTPS (server uses TLS)
    } else {
        args[0]
    };
    
    println!("🔌 Attempting to connect to gRPC server at: {}", addr);
    println!("   (This confirms we're using gRPC instead of the old BotApi)");
    
    match GrpcClient::connect(addr).await.map_err(|e| e.to_string()) {
        Ok(mut client) => {
            // Try a simple request to verify connectivity
            match client.user.list_users(maowbot_proto::maowbot::services::ListUsersRequest {
                page: Some(maowbot_proto::maowbot::common::PageRequest {
                    page_size: 1,
                    page_token: String::new(),
                }),
                filter: Some(maowbot_proto::maowbot::services::ListUsersFilter {
                    active_only: false,
                    platforms: vec![],
                    roles: vec![],
                }),
                order_by: "created_at".to_string(),
                descending: false,
            }).await {
                Ok(response) => {
                    let resp = response.into_inner();
                    let page_info = resp.page.unwrap_or_default();
                    format!(
                        "✅ gRPC connection successful!\n\
                        Server responded with:\n\
                        - Total users: {}\n\
                        - Response contains {} user(s)",
                        page_info.total_count,
                        resp.users.len()
                    )
                }
                Err(e) => format!("❌ gRPC call failed: {}", e),
            }
        }
        Err(e) => format!("❌ Failed to connect to gRPC server: {}", e),
    }
}