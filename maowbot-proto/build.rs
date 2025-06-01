fn main() {
    // Compile all protos together to resolve dependencies
    let mut protos = vec![
        "proto/plugin.proto",
        "proto/common.proto",
    ];
    
    let service_protos = vec![
        "proto/services/user_service.proto",
        "proto/services/credential_service.proto",
        "proto/services/platform_service.proto",
        "proto/services/plugin_service.proto",
        "proto/services/twitch_service.proto",
        "proto/services/discord_service.proto",
        "proto/services/vrchat_service.proto",
        "proto/services/osc_service.proto",
        "proto/services/config_service.proto",
        "proto/services/ai_service.proto",
        "proto/services/command_service.proto",
        "proto/services/redeem_service.proto",
    ];
    
    protos.extend(service_protos);
    
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&protos, &["proto"])
        .unwrap();
}