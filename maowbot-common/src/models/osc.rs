#[derive(Debug)]
pub struct OscStatus {
    pub is_running: bool,
    pub listening_port: Option<u16>,
    pub is_oscquery_running: bool,
    pub oscquery_port: Option<u16>,

    /// Optionally, any discovered local OSCQuery peers, if we've run a discovery check.
    pub discovered_peers: Vec<String>,
}