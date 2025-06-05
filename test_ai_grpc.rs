// Test program to verify AI service gRPC connectivity

use maowbot_proto::maowbot::services::{
    ai_service_client::AiServiceClient,
    ListProvidersRequest,
};
use tonic::transport::Channel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing AI Service gRPC Connection...");
    
    // Connect to the gRPC server
    let channel = Channel::from_static("https://127.0.0.1:9999")
        .tls_config(tonic::transport::ClientTlsConfig::new())?
        .connect()
        .await?;
    
    let mut client = AiServiceClient::new(channel);
    
    // Try to list providers
    println!("Calling list_providers...");
    let request = tonic::Request::new(ListProvidersRequest {
        configured_only: false,
    });
    
    match client.list_providers(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            println!("Success! Found {} providers", resp.providers.len());
            println!("Active provider: {}", resp.active_provider);
            for provider in resp.providers {
                println!("  - {} (configured: {}, active: {})", 
                    provider.name, provider.is_configured, provider.is_active);
            }
        }
        Err(e) => {
            println!("Error: {:?}", e);
            println!("Status code: {:?}", e.code());
            println!("Message: {}", e.message());
        }
    }
    
    Ok(())
}