use tonic::transport::{Channel, Endpoint};
use maowbot_proto::maowbot::services::{
    user_service_client::UserServiceClient,
    credential_service_client::CredentialServiceClient,
    platform_service_client::PlatformServiceClient,
    command_service_client::CommandServiceClient,
    redeem_service_client::RedeemServiceClient,
    config_service_client::ConfigServiceClient,
    ai_service_client::AiServiceClient,
    plugin_service_client::PluginServiceClient,
    osc_service_client::OscServiceClient,
    twitch_service_client::TwitchServiceClient,
    discord_service_client::DiscordServiceClient,
    vr_chat_service_client::VrChatServiceClient,
    autostart_service_client::AutostartServiceClient,
};
use std::time::Duration;

#[derive(Clone)]
pub struct GrpcClient {
    pub user: UserServiceClient<Channel>,
    pub credential: CredentialServiceClient<Channel>,
    pub platform: PlatformServiceClient<Channel>,
    pub command: CommandServiceClient<Channel>,
    pub redeem: RedeemServiceClient<Channel>,
    pub config: ConfigServiceClient<Channel>,
    pub ai: AiServiceClient<Channel>,
    pub plugin: PluginServiceClient<Channel>,
    pub osc: OscServiceClient<Channel>,
    pub twitch: TwitchServiceClient<Channel>,
    pub discord: DiscordServiceClient<Channel>,
    pub vrchat: VrChatServiceClient<Channel>,
    pub autostart: AutostartServiceClient<Channel>,
}

impl GrpcClient {
    pub async fn connect(addr: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Create a shared channel with connection pooling
        let endpoint = if addr.starts_with("https") {
            // For HTTPS with self-signed certs, configure TLS
            let mut tls = tonic::transport::ClientTlsConfig::new();
            
            // Try to load the server's certificate if it exists
            if let Ok(cert_pem) = std::fs::read("certs/server.crt") {
                let ca = tonic::transport::Certificate::from_pem(cert_pem);
                tls = tls.ca_certificate(ca);
            } else {
                // For self-signed certs without the CA, we need to skip verification
                // WARNING: This is insecure and should only be used for development!
                // Note: tonic doesn't support skipping verification directly
                // We'll need to load a dummy cert or use HTTP instead
            }
            
            Endpoint::from_shared(addr.to_string())?
                .tls_config(tls)?
                .timeout(Duration::from_secs(30))
                .connect_timeout(Duration::from_secs(10))
        } else {
            Endpoint::from_shared(addr.to_string())?
                .timeout(Duration::from_secs(30))
                .connect_timeout(Duration::from_secs(10))
        };
            
        let channel = endpoint.connect().await?;
        
        Ok(Self {
            user: UserServiceClient::new(channel.clone()),
            credential: CredentialServiceClient::new(channel.clone()),
            platform: PlatformServiceClient::new(channel.clone()),
            command: CommandServiceClient::new(channel.clone()),
            redeem: RedeemServiceClient::new(channel.clone()),
            config: ConfigServiceClient::new(channel.clone()),
            ai: AiServiceClient::new(channel.clone()),
            plugin: PluginServiceClient::new(channel.clone()),
            osc: OscServiceClient::new(channel.clone()),
            twitch: TwitchServiceClient::new(channel.clone()),
            discord: DiscordServiceClient::new(channel.clone()),
            vrchat: VrChatServiceClient::new(channel.clone()),
            autostart: AutostartServiceClient::new(channel.clone()),
        })
    }
    
    
    pub async fn connect_with_tls(
        addr: &str,
        ca_cert: &[u8],
        client_cert: &[u8],
        client_key: &[u8],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        use tonic::transport::{Certificate, ClientTlsConfig, Identity};
        
        let ca = Certificate::from_pem(ca_cert);
        let identity = Identity::from_pem(client_cert, client_key);
        
        let tls = ClientTlsConfig::new()
            .ca_certificate(ca)
            .identity(identity);
            
        let endpoint = Endpoint::from_shared(addr.to_string())?
            .tls_config(tls)?
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10));
            
        let channel = endpoint.connect().await?;
        
        Ok(Self {
            user: UserServiceClient::new(channel.clone()),
            credential: CredentialServiceClient::new(channel.clone()),
            platform: PlatformServiceClient::new(channel.clone()),
            command: CommandServiceClient::new(channel.clone()),
            redeem: RedeemServiceClient::new(channel.clone()),
            config: ConfigServiceClient::new(channel.clone()),
            ai: AiServiceClient::new(channel.clone()),
            plugin: PluginServiceClient::new(channel.clone()),
            osc: OscServiceClient::new(channel.clone()),
            twitch: TwitchServiceClient::new(channel.clone()),
            discord: DiscordServiceClient::new(channel.clone()),
            vrchat: VrChatServiceClient::new(channel.clone()),
            autostart: AutostartServiceClient::new(channel.clone()),
        })
    }
}