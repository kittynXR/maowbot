use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::pin::Pin;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::{Mutex, watch};
use warp::http::StatusCode;
use warp::{Filter, Rejection, Reply};
use tracing::{info, error};
use futures_util::stream::TryStreamExt;
use tokio_util::compat::TokioAsyncWriteCompatExt;

use crate::{Result, OscError};
use crate::oscquery::mdns::{MdnsService, AdvertisedService};
use crate::oscquery::models::{
    HostExtensions,
    OSCMethod,
    OSCMethodAccessType,
    OSCMethodValueType,
    OSCQueryHostInfo,
    OSCQueryNode
};

fn build_host_info(osc_port: u16, service_name: &str) -> OSCQueryHostInfo {
    OSCQueryHostInfo {
        NAME: service_name.to_owned(),
        EXTENSIONS: HostExtensions {
            ACCESS: true,
            CLIPMODE: false,
            RANGE: true,
            TYPE: true,
            VALUE: true,
        },
        OSC_IP: "127.0.0.1".to_string(),
        OSC_PORT: osc_port,
        OSC_TRANSPORT: "UDP".to_string(),
    }
}

pub struct OscQueryServer {
    pub is_running: bool,
    pub http_port: u16,
    pub osc_port: u16,

    // Arc/Mutex so Warp route closures can share the name without moving `self`.
    pub service_name: Arc<Mutex<Option<String>>>,

    pub mdns_service: Option<MdnsService>,

    // We store all discovered OSCMethod definitions here.
    pub methods: Arc<Mutex<Vec<OSCMethod>>>,

    // The “root node” for JSON data. Also in an Arc so Warp can read it.
    pub root_node: Arc<Mutex<Option<OSCQueryNode>>>,

    // Stop signal for the Warp server
    stop_tx: Option<watch::Sender<bool>>,

    // Handle for the spawned Warp server task
    server_task: Option<tokio::task::JoinHandle<()>>,
}

impl OscQueryServer {
    pub fn new(http_port: u16) -> Self {
        Self {
            is_running: false,
            http_port,
            osc_port: 9001,
            service_name: Arc::new(Mutex::new(None)),
            mdns_service: None,
            methods: Arc::new(Mutex::new(vec![])),
            root_node: Arc::new(Mutex::new(None)),
            stop_tx: None,
            server_task: None,
        }
    }

    pub fn set_osc_port(&mut self, port: u16) {
        self.osc_port = port;
    }

    /// Assign an explicit name for the service. If it’s never set, /HOST_INFO panics.
    pub async fn set_service_name(&self, name: &str) {
        let mut lock = self.service_name.lock().await;
        *lock = Some(name.to_owned());
    }

    /// Start the Warp HTTP server. If `self.http_port == 0`, we attempt ephemeral binding
    /// and store the chosen port in `self.http_port`.
    ///
    /// We do **not** move `self` into an Arc. Instead, we create local copies (or Arc clones)
    /// of the fields we need in the route closures, thereby avoiding lifetime errors.
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running {
            return Ok(());
        }
        self.is_running = true;

        // Create an mDNS service but don't start it until we want to advertise.
        let mdns = MdnsService::new()
            .map_err(|e| OscError::IoError(format!("Failed to create MdnsService: {e}")))?;
        self.mdns_service = Some(mdns);

        let (stop_tx, stop_rx) = watch::channel(false);
        self.stop_tx = Some(stop_tx);

        // --- Prepare local copies for Warp route closures ---
        // `u16` can be trivially cloned into each closure.
        let local_osc_port = self.osc_port;
        // We'll fill in the final `http_port` below (after ephemeral binding).
        // For now, store these arcs so each route closure can reference them:
        let root_node_arc = self.root_node.clone();
        let service_name_arc = self.service_name.clone();

        // Build the route for `/` (root) to return the entire node JSON
        let route_root = warp::path::end().and_then(move || {
            let node_ref = root_node_arc.clone();
            async move {
                let node_opt = node_ref.lock().await;
                if let Some(node) = &*node_opt {
                    match serde_json::to_string(node) {
                        Ok(json_str) => {
                            let reply = warp::reply::with_status(json_str, StatusCode::OK);
                            let reply = warp::reply::with_header(reply, "Content-Type", "application/json");
                            Ok::<_, Rejection>(reply)
                        }
                        Err(_) => {
                            let reply = warp::reply::with_status("".to_string(), StatusCode::NO_CONTENT);
                            let reply = warp::reply::with_header(reply, "Content-Type", "application/json");
                            Ok::<_, Rejection>(reply)
                        }
                    }
                } else {
                    let reply = warp::reply::with_status("".to_string(), StatusCode::NO_CONTENT);
                    let reply = warp::reply::with_header(reply, "Content-Type", "application/json");
                    Ok::<_, Rejection>(reply)
                }
            }
        });

        // Build the route for `/HOST_INFO`
        // We capture `local_osc_port` plus the shared `service_name_arc`.
        let route_host_info = warp::path("HOST_INFO").and_then(move || {
            let name_arc = service_name_arc.clone();
            async move {
                let lock = name_arc.lock().await;
                let service_name = lock
                    .as_ref()
                    .expect("IMPOSSIBLE CODEPATH: service_name is None"); // user requires no fallback

                let info = build_host_info(local_osc_port, service_name);
                match serde_json::to_string(&info) {
                    Ok(json_str) => {
                        let reply = warp::reply::with_status(json_str, StatusCode::OK);
                        let reply = warp::reply::with_header(reply, "Content-Type", "application/json");
                        Ok::<_, Rejection>(reply)
                    },
                    Err(_) => {
                        let reply = warp::reply::with_status("".into(), StatusCode::NO_CONTENT);
                        let reply = warp::reply::with_header(reply, "Content-Type", "application/json");
                        Ok::<_, Rejection>(reply)
                    },
                }
            }
        });

        let routes = route_root.or(route_host_info);
        let warp_server = warp::serve(routes);

        // Now do the actual binding. If `self.http_port == 0`, we want ephemeral.
        let server_future: Pin<Box<dyn Future<Output=()> + Send>> = if self.http_port == 0 {
            let (addr, fut) = warp_server.bind_ephemeral(([0, 0, 0, 0], 0));
            self.http_port = addr.port();
            info!("Starting OSCQuery HTTP server on ephemeral port {}", self.http_port);
            Box::pin(fut)
        } else {
            let addr = ([0, 0, 0, 0], self.http_port);
            info!("Starting OSCQuery HTTP server on port {}", self.http_port);
            Box::pin(async move {
                warp_server.run(addr).await;
            })
        };

        // Spawn the Warp server in a background task, watching for a stop signal
        let mut rx = stop_rx.clone();
        let join = tokio::spawn(async move {
            tokio::select! {
                _ = server_future => {
                    info!("Warp server finished normally");
                }
                _ = async {
                    while rx.changed().await.is_ok() {
                        if *rx.borrow() {
                            break;
                        }
                    }
                } => {
                    info!("OSCQuery server shutting down due to watch signal");
                }
            }
        });
        self.server_task = Some(join);

        // Build the initial root node from existing methods
        self.rebuild_root_node().await?;
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        if !self.is_running {
            return Ok(());
        }
        self.is_running = false;

        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(true);
        }
        if let Some(handle) = self.server_task.take() {
            handle.abort();
        }
        if let Some(ms) = self.mdns_service.as_mut() {
            ms.stop();
        }
        self.mdns_service = None;
        Ok(())
    }

    /// Advertise this service under a random "MAOW-XXXXXX" name, and store
    /// that name in `service_name` so `/HOST_INFO` can see it.
    pub async fn advertise_as_maow(&mut self) -> Result<()> {
        let random_hex = random_hex_6();
        let instance_name = format!("MAOW-{}", random_hex);

        {
            let mut lock = self.service_name.lock().await;
            *lock = Some(instance_name.clone());
        }

        if let Some(ms) = &self.mdns_service {
            ms.advertise(
                &instance_name,
                "_osc._udp.local.",
                self.osc_port,
                Ipv4Addr::new(10, 11, 11, 123),
            );
            ms.advertise(
                &instance_name,
                "_oscjson._tcp.local.",
                self.http_port,
                Ipv4Addr::new(127, 0, 0, 1),
            );
        }

        if let Some(ms) = &mut self.mdns_service {
            ms.start();
        }

        info!(
            "Advertisements active for '{instance_name}' => TCP:{}, UDP:{}",
            self.http_port, self.osc_port
        );
        Ok(())
    }

    pub async fn add_osc_method(&self, method: OSCMethod) -> Result<()> {
        {
            let mut locked = self.methods.lock().await;
            if let Some(i) = locked.iter().position(|m| m.address == method.address) {
                locked[i] = method;
            } else {
                locked.push(method);
            }
        }
        self.rebuild_root_node().await
    }

    pub async fn remove_osc_method(&self, address: &str) -> Result<()> {
        {
            let mut locked = self.methods.lock().await;
            locked.retain(|m| m.address != address);
        }
        self.rebuild_root_node().await
    }

    pub async fn set_osc_method_value(&self, address: &str, value: Option<String>) -> Result<()> {
        {
            let mut locked = self.methods.lock().await;
            if let Some(m) = locked.iter_mut().find(|m| m.address == address) {
                m.value = value;
            }
        }
        self.rebuild_root_node().await
    }

    pub async fn receive_vrchat_avatar_parameters(&self) -> Result<()> {
        self.add_osc_method(OSCMethod {
            address: "/avatar".into(),
            access_type: OSCMethodAccessType::Write,
            value_type: None,
            value: None,
            description: None,
        }).await
    }

    pub async fn receive_vrchat_tracking_data(&self) -> Result<()> {
        self.add_osc_method(OSCMethod {
            address: "/tracking/vrsystem".into(),
            access_type: OSCMethodAccessType::Write,
            value_type: None,
            value: None,
            description: None,
        }).await
    }

    /// Rebuild the entire root node from the currently known `methods`.
    async fn rebuild_root_node(&self) -> Result<()> {
        let methods = self.methods.lock().await.clone();
        let mut root = OSCQueryNode {
            DESCRIPTION: Some("root node".to_string()),
            FULL_PATH: "/".to_string(),
            ACCESS: 0,
            CONTENTS: HashMap::new(),
            TYPE: None,
            VALUE: vec![],
        };

        for m in methods {
            self.insert_method_into_node(&mut root, &m);
        }

        // Example: ensure /avatar/change is always present:
        if !root.CONTENTS.contains_key("avatar") {
            let mut avatar_change_contents = HashMap::new();
            avatar_change_contents.insert("change".to_string(), OSCQueryNode {
                DESCRIPTION: None,
                FULL_PATH: "/avatar/change".to_string(),
                ACCESS: 2,
                CONTENTS: HashMap::new(),
                TYPE: Some("s".to_string()),
                VALUE: vec![],
            });
            root.CONTENTS.insert("avatar".to_string(), OSCQueryNode {
                DESCRIPTION: None,
                FULL_PATH: "/avatar".to_string(),
                ACCESS: 2,
                CONTENTS: avatar_change_contents,
                TYPE: None,
                VALUE: vec![],
            });
        }

        let mut lock = self.root_node.lock().await;
        *lock = Some(root);
        Ok(())
    }

    fn insert_method_into_node(&self, root: &mut OSCQueryNode, method: &OSCMethod) {
        let parts: Vec<&str> = method.address
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        let mut current = root;
        let mut full_path = String::from("/");
        for (idx, part) in parts.iter().enumerate() {
            if idx > 0 {
                full_path.push('/');
            }
            full_path.push_str(part);
            current = current
                .CONTENTS
                .entry(part.to_string())
                .or_insert_with(|| OSCQueryNode {
                    DESCRIPTION: None,
                    FULL_PATH: full_path.clone(),
                    ACCESS: 0,
                    CONTENTS: Default::default(),
                    TYPE: None,
                    VALUE: vec![],
                });
        }

        // 1=Read, 2=Write, 3=ReadWrite
        current.ACCESS = match method.access_type {
            OSCMethodAccessType::Write => 2,
            OSCMethodAccessType::Read => 1,
            OSCMethodAccessType::ReadWrite => 3,
        };

        if let Some(vt) = method.value_type {
            current.TYPE = Some(vt.osc_type_str().to_string());
        }

        if let Some(ref val) = method.value {
            let vt = method.value_type.unwrap_or(OSCMethodValueType::String);
            match vt {
                OSCMethodValueType::Bool => {
                    current.VALUE = vec![serde_json::Value::Bool(val == "true")];
                }
                OSCMethodValueType::Int => {
                    if let Ok(n) = val.parse::<i64>() {
                        current.VALUE = vec![serde_json::Value::Number(n.into())];
                    }
                }
                OSCMethodValueType::Float => {
                    if let Ok(f) = val.parse::<f64>() {
                        if let Some(num) = serde_json::Number::from_f64(f) {
                            current.VALUE = vec![serde_json::Value::Number(num)];
                        }
                    }
                }
                OSCMethodValueType::String => {
                    current.VALUE = vec![serde_json::Value::String(val.clone())];
                }
            }
        }
    }
}

/// Helper for generating random 6-digit hex strings like "A1B2C3"
fn random_hex_6() -> String {
    use rand::Rng;
    let r: u32 = rand::thread_rng().gen_range(0..=0xFFFFFF);
    format!("{:06X}", r)
}
