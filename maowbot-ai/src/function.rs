use std::collections::HashMap;
use std::sync::Arc;

use serde::{Serialize, Deserialize};
use tokio::sync::RwLock;
use crate::models::FunctionParameter;

/// Function signature for AI callable functions
pub type FunctionHandler = Arc<dyn Fn(HashMap<String, serde_json::Value>) -> anyhow::Result<serde_json::Value> + Send + Sync>;

/// Represents a function that can be called by AI
#[derive(Clone)]
pub struct Function {
    /// Name of the function
    pub name: String,
    
    /// Description of what the function does
    pub description: String,
    
    /// Parameters that the function accepts
    pub parameters: Vec<FunctionParameter>,
    
    /// Function handler implementation
    pub handler: FunctionHandler,
}

/// Schema representation of a function for AI models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionSchema {
    /// Name of the function
    pub name: String,
    
    /// Description of what the function does
    pub description: String,
    
    /// Parameters schema in JSON Schema format
    pub parameters: serde_json::Value,
}

impl Function {
    /// Create a new function with specified name, description, parameters and handler
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Vec<FunctionParameter>,
        handler: FunctionHandler,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
            handler,
        }
    }
    
    /// Convert to schema representation for AI providers
    pub fn to_schema(&self) -> FunctionSchema {
        // Convert parameters to JSON Schema format
        let parameters_schema = self.build_parameters_schema();
        
        FunctionSchema {
            name: self.name.clone(),
            description: self.description.clone(),
            parameters: parameters_schema,
        }
    }
    
    /// Build JSON Schema representation of function parameters
    fn build_parameters_schema(&self) -> serde_json::Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();
        
        for param in &self.parameters {
            let mut param_schema = serde_json::Map::new();
            param_schema.insert("description".to_string(), serde_json::Value::String(param.description.clone()));
            param_schema.insert("type".to_string(), serde_json::Value::String(param.parameter_type.clone()));
            
            if let Some(default) = &param.default {
                param_schema.insert("default".to_string(), default.clone());
            }
            
            if let Some(enum_values) = &param.enum_values {
                let enum_array = serde_json::Value::Array(
                    enum_values.iter().map(|v| serde_json::Value::String(v.clone())).collect()
                );
                param_schema.insert("enum".to_string(), enum_array);
            }
            
            properties.insert(param.name.clone(), serde_json::Value::Object(param_schema));
            
            if param.required {
                required.push(serde_json::Value::String(param.name.clone()));
            }
        }
        
        let mut schema = serde_json::Map::new();
        schema.insert("type".to_string(), serde_json::Value::String("object".to_string()));
        schema.insert("properties".to_string(), serde_json::Value::Object(properties));
        
        if !required.is_empty() {
            schema.insert("required".to_string(), serde_json::Value::Array(required));
        }
        
        serde_json::Value::Object(schema)
    }
    
    /// Execute the function with the given arguments
    pub fn execute(&self, args: HashMap<String, serde_json::Value>) -> anyhow::Result<serde_json::Value> {
        // Call the handler with provided arguments
        (self.handler)(args)
    }
}

/// Registry of functions that can be called by AI
pub struct FunctionRegistry {
    functions: Arc<RwLock<HashMap<String, Function>>>,
}

impl FunctionRegistry {
    /// Create a new empty function registry
    pub fn new() -> Self {
        Self {
            functions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Register a new function
    pub async fn register(&self, function: Function) {
        let mut functions = self.functions.write().await;
        functions.insert(function.name.clone(), function);
    }
    
    /// Get a function by name
    pub async fn get(&self, name: &str) -> Option<Function> {
        let functions = self.functions.read().await;
        functions.get(name).cloned()
    }
    
    /// Get all registered functions
    pub async fn get_all(&self) -> Vec<Function> {
        let functions = self.functions.read().await;
        functions.values().cloned().collect()
    }
    
    /// Get function schemas for all registered functions
    pub async fn get_all_schemas(&self) -> Vec<FunctionSchema> {
        let functions = self.functions.read().await;
        functions.values().map(|f| f.to_schema()).collect()
    }
    
    /// Remove a function from the registry
    pub async fn remove(&self, name: &str) -> Option<Function> {
        let mut functions = self.functions.write().await;
        functions.remove(name)
    }
    
    /// Execute a function by name with the given arguments
    pub async fn execute(&self, name: &str, args: HashMap<String, serde_json::Value>) -> anyhow::Result<serde_json::Value> {
        let function = self.get(name).await.ok_or_else(|| {
            anyhow::anyhow!("Function not found: {}", name)
        })?;
        
        function.execute(args)
    }
}