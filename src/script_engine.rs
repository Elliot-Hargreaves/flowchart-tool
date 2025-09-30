//! Unified JavaScript script execution engine
//! 
//! This module provides JavaScript script execution capabilities that work both in WASM
//! and native environments using the boa pure-Rust JavaScript implementation.

use serde_json::Value;
use boa_engine::{Context, JsValue, Source, property::PropertyKey, JsString, object::builtins::JsArray, JsObject, js_string};
use boa_engine::property::NonMaxU32;

/// JavaScript script execution engine that works on all platforms
pub struct JavaScriptEngine {
    context: Context,
}

impl JavaScriptEngine {
    /// Create a new JavaScript script engine
    pub fn new() -> Result<Self, String> {
        let context = Context::default();
        Ok(Self { context })
    }

    /// Execute a JavaScript script with the given input data and return the result
    /// 
    /// The script receives the input as a global 'input' variable and should return a result.
    /// The input is provided as a parsed JavaScript object/value.
    pub fn execute(&mut self, script: &str, input: Value) -> Result<Value, String> {
        // Convert serde_json::Value to JsValue
        let js_input = self.json_to_js_value(&input)
            .map_err(|e| format!("Failed to convert input to JavaScript value: {}", e))?;

        // Set the input as a global variable
        let input_key = PropertyKey::String(JsString::from("input"));
        self.context
            .register_global_property(input_key, js_input, Default::default())
            .map_err(|e| format!("Failed to set input variable: {}", e))?;

        // Execute the JavaScript code
        let source = Source::from_bytes(script);
        let js_result = self.context
            .eval(source)
            .map_err(|e| format!("JavaScript execution error: {}", e))?;

        // Convert the result back to serde_json::Value
        self.js_value_to_json(&js_result)
            .map_err(|e| format!("Failed to convert result to JSON: {}", e))
    }

    /// Convert serde_json::Value to boa JsValue
    fn json_to_js_value(&mut self, value: &Value) -> Result<JsValue, String> {
        match value {
            Value::Null => Ok(JsValue::null()),
            Value::Bool(b) => Ok(JsValue::from(*b)),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(JsValue::from(i as i32))
                } else if let Some(f) = n.as_f64() {
                    Ok(JsValue::from(f))
                } else {
                    Err("Invalid number format".to_string())
                }
            },
            Value::String(s) => Ok(JsValue::String(JsString::from(s.as_str()))),
            Value::Array(arr) => {
                // Create array using JsArray::new
                let js_array = JsArray::new(&mut self.context);

                for (index, item) in arr.iter().enumerate() {
                    let js_item = self.json_to_js_value(item)?;
                    js_array.set(index as u32, js_item, false, &mut self.context)
                        .map_err(|e| format!("Failed to set array element: {}", e))?;
                }
                Ok(js_array.into())
            },
            Value::Object(obj) => {
                // Create object using Object::create
                let js_obj = JsObject::default();

                for (key, val) in obj {
                    let js_val = self.json_to_js_value(val)?;
                    js_obj.set(JsString::from(key.as_str()), js_val, false, &mut self.context)
                        .map_err(|e| format!("Failed to set object property: {}", e))?;
                }
                Ok(JsValue::Object(js_obj))
            }
        }
    }

    /// Convert boa JsValue to serde_json::Value
    fn js_value_to_json(&mut self, js_value: &JsValue) -> Result<Value, String> {
        match js_value {
            JsValue::Null => Ok(Value::Null),
            JsValue::Undefined => Ok(Value::Null),
            JsValue::Boolean(b) => Ok(Value::Bool(*b)),
            JsValue::String(s) => {
                let string_val = s.to_std_string()
                    .map_err(|_| "Failed to convert JS string to Rust string")?;
                Ok(Value::String(string_val))
            },
            JsValue::Rational(r) => Ok(serde_json::Number::from_f64(*r)
                .map(Value::Number)
                .unwrap_or(Value::Null)),
            JsValue::Integer(i) => Ok(Value::Number(serde_json::Number::from(*i))),
            JsValue::BigInt(_) => Err("BigInt not supported".to_string()),
            JsValue::Object(obj) => {
                // Check if this is an array
                if obj.is_array() {
                    let mut array = Vec::new();
                    let length_property = obj.get(js_string!("length"), &mut self.context)
                        .map_err(|e| format!("Failed to get array length: {}", e))?;

                    let length = if let JsValue::Integer(len) = length_property {
                        len as u32
                    } else if let JsValue::Rational(len) = length_property {
                        len as u32
                    } else {
                        0u32
                    };

                    for i in 0..length {
                        let element = obj.get(PropertyKey::Index(NonMaxU32::new(i).unwrap()), &mut self.context)
                            .map_err(|e| format!("Failed to get array element: {}", e))?;
                        array.push(self.js_value_to_json(&element)?);
                    }
                    Ok(Value::Array(array))
                } else {
                    // For regular objects, we'll return a simplified version
                    // A full implementation would need to enumerate all properties
                    let map = serde_json::Map::new();
                    Ok(Value::Object(map))
                }
            },
            JsValue::Symbol(_) => Err("Symbol not supported".to_string()),
        }
    }
}

/// Create a JavaScript script engine
pub fn create_script_engine() -> Result<JavaScriptEngine, String> {
    JavaScriptEngine::new()
}
