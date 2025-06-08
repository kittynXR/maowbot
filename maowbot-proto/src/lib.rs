pub mod plugs {
    tonic::include_proto!("plugs");
}

pub mod maowbot {
    pub mod common {
        tonic::include_proto!("maowbot.common");
    }
    
    pub mod services {
        tonic::include_proto!("maowbot.services");
        
        pub mod event_pipeline {
            tonic::include_proto!("maowbot_proto.services.event_pipeline");
        }
    }
}

// Re-export prost_types for convenience
pub use prost_types;