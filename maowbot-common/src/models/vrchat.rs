#[derive(Debug)]
pub struct VRChatWorldBasic {
    pub name: String,
    pub author_name: String,
    pub updated_at: String,
    pub created_at: String,
    pub capacity: u32,

    /// New: textual "release status" from VRChat (e.g. "public", "private", "hidden", "all ...", "communityLabs").
    pub release_status: String,

    /// Optional description if present
    pub description: String,
}

/// Basic fields representing a VRChat instance.
#[derive(Debug)]
pub struct VRChatInstanceBasic {
    pub world_id: Option<String>,
    pub instance_id: Option<String>,
    pub location: Option<String>,
}

/// Basic fields representing a VRChat avatar.
#[derive(Debug)]
pub struct VRChatAvatarBasic {
    pub avatar_id: String,
    pub avatar_name: String,
}