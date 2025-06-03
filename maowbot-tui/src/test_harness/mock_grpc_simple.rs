use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tonic::{Request, Response, Status};
use prost::Message;

pub type ResponseCallback = Arc<dyn Fn(&str, &[u8]) -> Result<Vec<u8>, Status> + Send + Sync>;

#[derive(Clone)]
pub struct SimpleMockGrpcClient {
    responses: Arc<Mutex<HashMap<String, ResponseCallback>>>,
    call_log: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
}

impl SimpleMockGrpcClient {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(HashMap::new())),
            call_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn with_response<F>(self, method: &str, callback: F) -> Self
    where
        F: Fn(&str, &[u8]) -> Result<Vec<u8>, Status> + Send + Sync + 'static,
    {
        self.responses.lock().unwrap().insert(method.to_string(), Arc::new(callback));
        self
    }

    pub fn with_static_response<T: Message>(self, method: &str, response: T) -> Self {
        let response_bytes = response.encode_to_vec();
        self.with_response(method, move |_, _| Ok(response_bytes.clone()))
    }

    pub fn with_error(self, method: &str, status: Status) -> Self {
        self.with_response(method, move |_, _| Err(status.clone()))
    }

    pub fn get_calls(&self) -> Vec<(String, Vec<u8>)> {
        self.call_log.lock().unwrap().clone()
    }

    pub fn clear_calls(&mut self) {
        self.call_log.lock().unwrap().clear();
    }

    pub async fn handle_call<Req, Res>(&self, method: &str, request: Request<Req>) -> Result<Response<Res>, Status>
    where
        Req: Message,
        Res: Message + Default,
    {
        let request_bytes = request.into_inner().encode_to_vec();
        self.call_log.lock().unwrap().push((method.to_string(), request_bytes.clone()));

        let responses = self.responses.lock().unwrap();
        if let Some(callback) = responses.get(method) {
            let response_bytes = callback(method, &request_bytes)?;
            let response = Res::decode(&response_bytes[..])
                .map_err(|e| Status::internal(format!("Failed to decode response: {}", e)))?;
            Ok(Response::new(response))
        } else {
            Err(Status::unimplemented(format!("Method {} not mocked", method)))
        }
    }
}