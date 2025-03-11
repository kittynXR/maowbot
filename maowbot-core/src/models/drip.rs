use std::sync::Mutex;
use sqlx::{Pool, Postgres};
use uuid::Uuid;

/// A minimal struct for storing an avatar row in memory.
#[derive(Clone, Debug)]
pub struct DripAvatar {
    pub drip_avatar_id: Uuid,
    pub user_id: Uuid,
    pub vrchat_avatar_id: String,
    pub vrchat_avatar_name: String,
    pub local_name: Option<String>,
}

/// A minimal struct for fits
#[derive(Clone, Debug)]
pub struct DripFitParam {
    pub param_name: String,
    pub param_value: String,
}

/// Example struct representing a row in `drip_fits`.
#[derive(Clone, Debug)]
pub struct DripFit {
    pub drip_fit_id: Uuid,
    pub drip_avatar_id: Uuid,
    pub fit_name: String,
}

/// Represents a row in `drip_props`
#[derive(Clone, Debug)]
pub struct DripProp {
    pub drip_prop_id: Uuid,
    pub prop_name: String,
}
