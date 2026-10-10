use serde_json::{Map, Value};

pub fn create_success_result(id: Value, result: Value) -> Value {
    let mut response = Map::new();
    response.insert("jsonrpc".into(), Value::String("2.0".into()));
    response.insert("id".into(), id);
    response.insert("result".into(), result);
    Value::Object(response)
}

pub fn create_notification(method: &str, params: Option<Value>) -> Value {
    let mut request = Map::new();
    request.insert("jsonrpc".into(), Value::String("2.0".into()));
    request.insert("method".into(), Value::String(method.into()));
    if let Some(params) = params {
        request.insert("params".into(), Value::Array(vec![params]));
    }
    Value::Object(request)
}

pub fn create_error(id: Value, message: &str, error_code: i32, data: Option<&str>) -> Value {
    let mut error = Map::new();
    error.insert("code".into(), Value::from(error_code));
    error.insert("message".into(), Value::String(message.into()));
    if let Some(data) = data.filter(|d| !d.trim().is_empty()) {
        error.insert("data".into(), Value::String(data.into()));
    }
    let mut response = Map::new();
    response.insert("jsonrpc".into(), Value::String("2.0".into()));
    response.insert("id".into(), id);
    response.insert("error".into(), Value::Object(error));
    Value::Object(response)
}

#[derive(Clone, Copy)]
pub enum JsonRpcErrors {
    ParseError,
    InvalidRequest,
    MethodNotFound,
    InvalidParams,
    InternalError,
}

impl JsonRpcErrors {
    fn code_and_message(self) -> (i32, &'static str) {
        match self {
            JsonRpcErrors::ParseError => (-32700, "Parse error"),
            JsonRpcErrors::InvalidRequest => (-32600, "Invalid Request"),
            JsonRpcErrors::MethodNotFound => (-32601, "Method not found"),
            JsonRpcErrors::InvalidParams => (-32602, "Invalid params"),
            JsonRpcErrors::InternalError => (-32603, "Internal error"),
        }
    }

    pub fn create_with_unknown_id(self, data: Option<&str>) -> Value {
        let (code, message) = self.code_and_message();
        create_error(Value::Null, message, code, data)
    }

    pub fn create_without_data(self, id: Value) -> Value {
        let (code, message) = self.code_and_message();
        create_error(id, message, code, None)
    }

    pub fn create(self, id: Value, data: &str) -> Value {
        let (code, message) = self.code_and_message();
        create_error(id, message, code, Some(data))
    }
}
