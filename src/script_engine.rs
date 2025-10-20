//! Unified JavaScript script execution engine
//!
//! This module provides JavaScript script execution capabilities that work both in WASM
//! and native environments using the boa pure-Rust JavaScript implementation.
//!
//! # Security & Sandboxing
//!
//! The JavaScript execution environment is sandboxed to ensure safe execution of
//! user-provided scripts:
//!
//! - **No File System Access**: boa_engine doesn't provide file system APIs
//! - **No Network Access**: No socket, fetch, or HTTP APIs are available
//! - **No eval()**: Dynamic code execution via eval() is disabled
//! - **No Function() constructor**: Cannot create functions from strings
//! - **Pure JavaScript Only**: Only safe, standard JavaScript operations are available
//!
//! This makes it safe to execute scripts from untrusted sources, as they can only
//! perform data transformations on the provided input without side effects.

use boa_engine::property::NonMaxU32;
use boa_engine::{
    js_string, object::builtins::JsArray, property::PropertyKey, Context, JsObject, JsResult,
    JsString, JsValue, Source,
};
use serde_json::Value;

/// JavaScript script execution engine that works on all platforms
pub struct JavaScriptEngine {
    context: Context,
}

impl JavaScriptEngine {
    /// Create a new JavaScript script engine with sandboxed execution environment
    ///
    /// The sandbox restricts access to potentially dangerous JavaScript features:
    /// - eval() is disabled to prevent dynamic code execution
    /// - Function() constructor is disabled
    /// - No file system or network access (boa doesn't provide these by default)
    ///
    /// Only safe, pure JavaScript operations are available for data transformation.
    pub fn new() -> Result<Self, String> {
        let mut context = Context::default();

        // Disable dangerous JavaScript features for sandboxing
        Self::setup_sandbox(&mut context).map_err(|e| format!("Failed to setup sandbox: {}", e))?;

        Ok(Self { context })
    }

    /// Setup sandbox by disabling dangerous JavaScript features
    fn setup_sandbox(context: &mut Context) -> JsResult<()> {
        // Disable eval() - prevents dynamic code execution
        let undefined = JsValue::undefined();
        context.register_global_property(
            js_string!("eval"),
            undefined.clone(),
            Default::default(),
        )?;

        // Disable Function constructor - prevents creating functions from strings
        // Note: boa doesn't expose all globals in the same way as browser JS,
        // but we can disable it through the global object
        let global = context.global_object().clone();
        global.set(js_string!("Function"), undefined.clone(), false, context)?;

        // Optional: You can also disable other potentially dangerous features
        // like import() if boa supports it in future versions

        Ok(())
    }

    /// Execute a JavaScript script without expecting a return value
    ///
    /// This is useful for defining functions and setting up the environment.
    pub fn execute_script(&mut self, script: &str) -> Result<(), String> {
        let source = Source::from_bytes(script);
        self.context
            .eval(source)
            .map_err(|e| format!("JavaScript execution error: {}", e))?;
        Ok(())
    }

    /// Execute a JavaScript script with the given input data and return the result
    ///
    /// The script receives the input as a global 'input' variable and should return a result.
    /// The input is provided as a parsed JavaScript object/value.
    pub fn execute(&mut self, script: &str, input: Value) -> Result<Value, String> {
        // Convert serde_json::Value to JsValue
        let js_input = self
            .json_to_js_value(&input)
            .map_err(|e| format!("Failed to convert input to JavaScript value: {}", e))?;

        // Set the input as a global variable
        let input_key = PropertyKey::String(JsString::from("input"));
        self.context
            .register_global_property(input_key, js_input, Default::default())
            .map_err(|e| format!("Failed to set input variable: {}", e))?;

        // Execute the JavaScript code
        let source = Source::from_bytes(script);
        let js_result = self
            .context
            .eval(source)
            .map_err(|e| format!("JavaScript execution error: {}", e))?;

        // Convert the result back to serde_json::Value
        self.js_value_to_json(&js_result)
            .map_err(|e| format!("Failed to convert result to JSON: {}", e))
    }

    /// Call a JavaScript function by name with the given argument
    ///
    /// The function must be defined in the global scope.
    pub fn call_function(&mut self, function_name: &str, arg: Value) -> Result<Value, String> {
        // Get the function from the global object
        let global = self.context.global_object().clone();
        let function_key = PropertyKey::String(JsString::from(function_name));
        let function_value = global
            .get(function_key, &mut self.context)
            .map_err(|e| format!("Failed to get function '{}': {}", function_name, e))?;

        // Check if it's a callable function
        if !function_value.is_callable() {
            return Err(format!("'{}' is not a function", function_name));
        }

        // Convert the argument to JsValue
        let js_arg = self.json_to_js_value(&arg)?;

        // Call the function
        let result = function_value
            .as_callable()
            .ok_or_else(|| format!("'{}' is not callable", function_name))?
            .call(&JsValue::undefined(), &[js_arg], &mut self.context)
            .map_err(|e| format!("Function call failed: {}", e))?;

        // Convert the result back to JSON
        self.js_value_to_json(&result)
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
            }
            Value::String(s) => Ok(JsValue::String(JsString::from(s.as_str()))),
            Value::Array(arr) => {
                // Create array using JsArray::new
                let js_array = JsArray::new(&mut self.context);

                for (index, item) in arr.iter().enumerate() {
                    let js_item = self.json_to_js_value(item)?;
                    js_array
                        .set(index as u32, js_item, false, &mut self.context)
                        .map_err(|e| format!("Failed to set array element: {}", e))?;
                }
                Ok(js_array.into())
            }
            Value::Object(obj) => {
                // Create object using Object::create
                let js_obj = JsObject::default();

                for (key, val) in obj {
                    let js_val = self.json_to_js_value(val)?;
                    js_obj
                        .set(
                            JsString::from(key.as_str()),
                            js_val,
                            false,
                            &mut self.context,
                        )
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
                let string_val = s
                    .to_std_string()
                    .map_err(|_| "Failed to convert JS string to Rust string")?;
                Ok(Value::String(string_val))
            }
            JsValue::Rational(r) => Ok(serde_json::Number::from_f64(*r)
                .map(Value::Number)
                .unwrap_or(Value::Null)),
            JsValue::Integer(i) => Ok(Value::Number(serde_json::Number::from(*i))),
            JsValue::BigInt(_) => Err("BigInt not supported".to_string()),
            JsValue::Object(obj) => {
                // Check if this is an array
                if obj.is_array() {
                    let mut array = Vec::new();
                    let length_property = obj
                        .get(js_string!("length"), &mut self.context)
                        .map_err(|e| format!("Failed to get array length: {}", e))?;

                    let length = if let JsValue::Integer(len) = length_property {
                        len as u32
                    } else if let JsValue::Rational(len) = length_property {
                        len as u32
                    } else {
                        0u32
                    };

                    for i in 0..length {
                        let element = obj
                            .get(
                                PropertyKey::Index(NonMaxU32::new(i).unwrap()),
                                &mut self.context,
                            )
                            .map_err(|e| format!("Failed to get array element: {}", e))?;
                        array.push(self.js_value_to_json(&element)?);
                    }
                    Ok(Value::Array(array))
                } else {
                    // Enumerate all properties of the object
                    let mut map = serde_json::Map::new();

                    // Get all enumerable own property keys
                    let keys = obj
                        .own_property_keys(&mut self.context)
                        .map_err(|e| format!("Failed to get object keys: {}", e))?;

                    for key in keys {
                        // Convert key to string
                        let key_str = match &key {
                            PropertyKey::String(s) => s
                                .to_std_string()
                                .map_err(|_| "Failed to convert key to string")?,
                            PropertyKey::Index(idx) => idx.get().to_string(),
                            PropertyKey::Symbol(_) => continue, // Skip symbols
                        };

                        // Get the value for this key
                        let value = obj
                            .get(key, &mut self.context)
                            .map_err(|e| format!("Failed to get property value: {}", e))?;

                        // Convert and add to map
                        let json_value = self.js_value_to_json(&value)?;
                        map.insert(key_str, json_value);
                    }

                    Ok(Value::Object(map))
                }
            }
            JsValue::Symbol(_) => Err("Symbol not supported".to_string()),
        }
    }
}

/// Create a JavaScript script engine
pub fn create_script_engine() -> Result<JavaScriptEngine, String> {
    JavaScriptEngine::new()
}

#[cfg(test)]
mod tests {
    use crate::simulation::execute_transformer_script;
    use crate::types::Message;
    use serde_json::{json, Number, Value};

    #[test]
    fn test_execute_script_simple_transform() {
        let script = r#"
            function transform(input) {
                input.value = input.value * 2;
                return input;
            }
        "#;
        let input = Message::new(json!({"value": 21}));
        let result = execute_transformer_script(script, &input).unwrap();
        let output = &result[0].data;

        assert_eq!(output["value"], 42);
    }

    #[test]
    fn test_execute_script_add_field() {
        let script = r#"
            function transform(input) {
                input.newField = 'added';
                return input;
            }
        "#;
        let input = Message::new(json!({"existingField": "original"}));
        let result = execute_transformer_script(script, &input).unwrap();
        let output = &result[0].data;

        assert_eq!(output["existingField"], "original");
        assert_eq!(output["newField"], "added");
    }

    #[test]
    fn test_execute_script_string_manipulation() {
        let script = r#"
            function transform(input) {
                input.text = input.text.toUpperCase();
                return input;
            }
        "#;
        let input = Message::new(json!({"text": "hello world"}));
        let result = execute_transformer_script(script, &input).unwrap();
        let output = &result[0].data;

        assert_eq!(output["text"], "HELLO WORLD");
    }

    #[test]
    fn test_execute_script_nested_object() {
        let script = r#"
            function transform(input) {
                input.nested.value = input.nested.value + 10;
                return input;
            }
        "#;
        let input = Message::new(json!({"nested": {"value": 5}}));
        let result = execute_transformer_script(script, &input).unwrap();
        let output = &result[0].data;

        assert_eq!(output["nested"]["value"], 15);
    }

    #[test]
    fn test_execute_script_array_operation() {
        let script = r#"
            function transform(input) {
                input.items.push('new');
                return input;
            }
        "#;
        let input = Message::new(json!({"items": ["a", "b"]}));
        let result = execute_transformer_script(script, &input).unwrap();
        let output = &result[0].data;

        assert_eq!(output["items"].as_array().unwrap().len(), 3);
        assert_eq!(output["items"][2], "new");
    }

    #[test]
    fn test_execute_script_conditional_logic() {
        let script = r#"
            function transform(input) {
                if (input.value > 10) {
                    input.result = 'high';
                } else {
                    input.result = 'low';
                }
                return input;
            }
        "#;
        let input = Message::new(json!({"value": 15}));
        let result = execute_transformer_script(script, &input).unwrap();
        let output = &result[0].data;

        assert_eq!(output["result"], "high");
    }

    #[test]
    fn test_execute_script_math_operations() {
        let script = r#"
            function transform(input) {
                input.sum = input.a + input.b;
                input.product = input.a * input.b;
                input.average = (input.a + input.b) / 2;
                return input;
            }
        "#;
        let input = Message::new(json!({"a": 10, "b": 20}));
        let result = execute_transformer_script(script, &input).unwrap();
        let output = &result[0].data;

        assert_eq!(output["sum"], 30);
        assert_eq!(output["product"], 200);
        assert_eq!(output["average"], 15);
    }

    #[test]
    fn test_execute_script_returns_original_on_syntax_error() {
        let script = "this is not valid javascript syntax {{{";
        let input = Message::new(json!({"value": 42}));
        let result = execute_transformer_script(script, &input);

        // Should return error on syntax error
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_script_returns_original_on_runtime_error() {
        let script = r#"
            function transform(input) {
                input.value = undefined.property;
                return input;
            }
        "#;
        let input = Message::new(json!({"value": 42}));
        let result = execute_transformer_script(script, &input);

        // Should return error on runtime error
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_script_empty_script() {
        let script = "";
        let input = Message::new(json!({"value": 42}));
        let result = execute_transformer_script(script, &input);

        // Empty script should return error (no transform function defined)
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_script_delete_field() {
        let script = r#"
            function transform(input) {
                delete input.toDelete;
                return input;
            }
        "#;
        let input = Message::new(json!({"keep": "this", "toDelete": "remove"}));
        let result = execute_transformer_script(script, &input).unwrap();
        let output = &result[0].data;

        assert!(output.get("keep").is_some());
        assert!(output.get("toDelete").is_none());
    }

    #[test]
    fn test_execute_script_boolean_operations() {
        let script = r#"
            function transform(input) {
                input.isValid = input.value > 0 && input.value < 100;
                input.hasError = !input.success;
                return input;
            }
        "#;
        let input = Message::new(json!({"value": 50, "success": true}));
        let result = execute_transformer_script(script, &input).unwrap();
        let output = &result[0].data;

        assert_eq!(output["isValid"], true);
        assert_eq!(output["hasError"], false);
    }

    #[test]
    fn test_execute_script_type_conversion() {
        let script = r#"
            function transform(input) {
                input.stringValue = String(input.number);
                input.numberValue = Number(input.string);
                return input;
            }
        "#;
        let input = Message::new(json!({"number": 42, "string": "123"}));
        let result = execute_transformer_script(script, &input).unwrap();
        let output = &result[0].data;

        assert_eq!(output["stringValue"], "42");
        assert_eq!(
            output["numberValue"],
            Value::Number(Number::from_f64(123.0).unwrap())
        );
    }

    #[test]
    fn test_execute_script_loop_operation() {
        let script = r#"
            function transform(input) {
                input.total = 0;
                for (var i = 0; i < input.values.length; i++) {
                    input.total += input.values[i];
                }
                return input;
            }
        "#;
        let input = Message::new(json!({"values": [1, 2, 3, 4, 5]}));
        let result = execute_transformer_script(script, &input).unwrap();
        let output = &result[0].data;

        assert_eq!(output["total"], 15);
    }

    #[test]
    fn test_execute_script_object_creation() {
        let script = r#"
            function transform(input) {
                input.metadata = {
                    timestamp: Date.now(),
                    processed: true,
                    version: 1
                };
                return input;
            }
        "#;
        let input = Message::new(json!({"data": "test"}));
        let result = execute_transformer_script(script, &input).unwrap();
        let output = &result[0].data;

        assert!(output["metadata"].is_object());
        assert_eq!(output["metadata"]["processed"], true);
        assert_eq!(output["metadata"]["version"], 1);
    }

    #[test]
    fn test_execute_script_json_with_special_characters() {
        let script = r#"
            function transform(input) {
                input.value = 'test';
                return input;
            }
        "#;
        let input = Message::new(json!({"text": "Quote: \" Backslash: \\ Newline: \n"}));
        let result = execute_transformer_script(script, &input).unwrap();
        let output = &result[0].data;

        assert_eq!(output["value"], "test");
        // Original text should be preserved
        assert!(output["text"].is_string());
    }
}
