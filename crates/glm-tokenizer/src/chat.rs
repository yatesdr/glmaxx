use std::fmt;

use serde::{Deserialize, Serialize};

use crate::OrderedValue;

const TOOL_PREAMBLE: &str = "<|system|>\n\
# Tools\n\n\
You may call one or more functions to assist with the user query.\n\n\
You are provided with function signatures within <tools></tools> XML tags:\n\
<tools>\n";
const TOOL_POSTAMBLE: &str = "</tools>\n\n\
For each function call, output the function name and arguments within the following XML format:\n\
<tool_call>{function-name}<arg_key>{arg-key-1}</arg_key><arg_value>{arg-value-1}</arg_value><arg_key>{arg-key-2}</arg_key><arg_value>{arg-value-2}</arg_value>...</tool_call>";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChatFunctionCall {
    pub name: String,
    pub arguments: OrderedValue,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChatToolCall {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    pub function: ChatFunctionCall,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChatMessage {
    pub role: ChatRole,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub reasoning_content: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<ChatToolCall>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub tool_call_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    High,
    #[default]
    Max,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChatTemplateOptions {
    pub reasoning_effort: ReasoningEffort,
    pub enable_thinking: bool,
    pub clear_thinking: Option<bool>,
    pub add_generation_prompt: bool,
}

impl Default for ChatTemplateOptions {
    fn default() -> Self {
        Self {
            reasoning_effort: ReasoningEffort::Max,
            enable_thinking: true,
            clear_thinking: None,
            add_generation_prompt: true,
        }
    }
}

pub fn render_chat(
    messages: &[ChatMessage],
    tools: Option<&[OrderedValue]>,
    options: ChatTemplateOptions,
) -> Result<String, ChatTemplateError> {
    if messages.is_empty() {
        return Err(ChatTemplateError::Messages);
    }
    if messages.iter().any(|message| {
        message.content.contains('\0')
            || message
                .reasoning_content
                .as_ref()
                .is_some_and(|value| value.contains('\0'))
            || message
                .name
                .as_ref()
                .is_some_and(|value| value.contains('\0'))
            || message
                .tool_call_id
                .as_ref()
                .is_some_and(|value| value.contains('\0'))
            || message.tool_calls.iter().any(|call| {
                call.function.name.contains('\0') || call.function.arguments.contains_nul()
            })
    }) || tools.is_some_and(|tools| tools.iter().any(OrderedValue::contains_nul))
    {
        return Err(ChatTemplateError::Messages);
    }
    let last_user = messages
        .iter()
        .rposition(|message| message.role == ChatRole::User);
    let mut output = String::from("[gMASK]<sop>");
    if options.enable_thinking {
        output.push_str("<|system|>Reasoning Effort: ");
        output.push_str(match options.reasoning_effort {
            ReasoningEffort::High => "High",
            ReasoningEffort::Max => "Max",
        });
    }
    if let Some(tools) = tools.filter(|tools| !tools.is_empty()) {
        output.push_str(TOOL_PREAMBLE);
        for tool in tools {
            output.push_str(&render_tool_definition(tool)?);
            output.push('\n');
        }
        output.push_str(TOOL_POSTAMBLE);
    }

    for (index, message) in messages.iter().enumerate() {
        match message.role {
            ChatRole::System => {
                require_content(message)?;
                reject_assistant_fields(message)?;
                output.push_str("<|system|>");
                output.push_str(&message.content);
            }
            ChatRole::User => {
                require_content(message)?;
                reject_assistant_fields(message)?;
                output.push_str("<|user|>");
                output.push_str(&message.content);
            }
            ChatRole::Assistant => {
                output.push_str("<|assistant|>");
                let (reasoning, content) = assistant_parts(message);
                let preserve_reasoning = options.clear_thinking == Some(false)
                    || last_user.is_none_or(|last_user| index > last_user);
                if preserve_reasoning && reasoning.is_some() {
                    output.push_str("<think>");
                    output.push_str(reasoning.unwrap_or_default());
                    output.push_str("</think>");
                } else {
                    output.push_str("<think></think>");
                }
                let content = content.trim();
                if !content.is_empty() {
                    output.push_str(content);
                }
                for tool_call in &message.tool_calls {
                    validate_tag_text(&tool_call.function.name)?;
                    output.push_str("<tool_call>");
                    output.push_str(&tool_call.function.name);
                    let parsed_arguments;
                    let arguments = if let Some(arguments) = tool_call.function.arguments.object() {
                        arguments
                    } else if let Some(arguments) = tool_call.function.arguments.string() {
                        parsed_arguments = serde_json::from_str::<OrderedValue>(arguments)
                            .map_err(|_| ChatTemplateError::ToolArguments)?;
                        parsed_arguments
                            .object()
                            .ok_or(ChatTemplateError::ToolArguments)?
                    } else {
                        return Err(ChatTemplateError::ToolArguments);
                    };
                    for (key, value) in arguments {
                        validate_tag_text(key)?;
                        output.push_str("<arg_key>");
                        output.push_str(key);
                        output.push_str("</arg_key><arg_value>");
                        if let Some(value) = value.string() {
                            output.push_str(value);
                        } else {
                            output.push_str(&value.python_json()?);
                        }
                        output.push_str("</arg_value>");
                    }
                    output.push_str("</tool_call>");
                }
            }
            ChatRole::Tool => {
                require_content(message)?;
                reject_assistant_fields(message)?;
                if index == 0 || messages[index - 1].role != ChatRole::Tool {
                    output.push_str("<|observation|>");
                }
                output.push_str("<tool_response>");
                output.push_str(&message.content);
                output.push_str("</tool_response>");
            }
        }
    }
    if options.add_generation_prompt {
        output.push_str("<|assistant|>");
        output.push_str(if options.enable_thinking {
            "<think>"
        } else {
            "<think></think>"
        });
    }
    Ok(output)
}

fn render_tool_definition(tool: &OrderedValue) -> Result<String, ChatTemplateError> {
    let selected = tool.get("function").unwrap_or(tool);
    let entries = selected.object().ok_or(ChatTemplateError::ToolDefinition)?;
    let mut output = String::from("{");
    let mut first = true;
    for (key, value) in entries {
        if matches!(key.as_str(), "defer_loading" | "strict") {
            continue;
        }
        validate_json_key(key)?;
        if !first {
            output.push_str(", ");
        }
        first = false;
        output.push('"');
        output.push_str(key);
        output.push_str("\": ");
        output.push_str(&value.python_json()?);
    }
    output.push('}');
    Ok(output)
}

fn assistant_parts(message: &ChatMessage) -> (Option<&str>, &str) {
    if let Some(reasoning) = message.reasoning_content.as_deref() {
        return (Some(reasoning), &message.content);
    }
    if let Some((before, after)) = message.content.split_once("</think>") {
        let reasoning = before
            .rsplit_once("<think>")
            .map_or(before, |(_, value)| value);
        let content = message
            .content
            .rsplit_once("</think>")
            .map_or(after, |(_, value)| value);
        return (Some(reasoning), content);
    }
    (None, &message.content)
}

fn require_content(message: &ChatMessage) -> Result<(), ChatTemplateError> {
    if message.content.is_empty() || message.content.contains('\0') {
        Err(ChatTemplateError::Messages)
    } else {
        Ok(())
    }
}

fn reject_assistant_fields(message: &ChatMessage) -> Result<(), ChatTemplateError> {
    if message.reasoning_content.is_some() || !message.tool_calls.is_empty() {
        Err(ChatTemplateError::Messages)
    } else {
        Ok(())
    }
}

fn validate_json_key(key: &str) -> Result<(), ChatTemplateError> {
    if key
        .chars()
        .any(|character| character == '"' || character == '\\' || character.is_control())
    {
        Err(ChatTemplateError::ToolDefinition)
    } else {
        Ok(())
    }
}

fn validate_tag_text(value: &str) -> Result<(), ChatTemplateError> {
    if value.contains('<') || value.contains('\0') {
        Err(ChatTemplateError::ToolArguments)
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChatTemplateError {
    Messages,
    ToolDefinition,
    ToolArguments,
    Json(String),
}

impl fmt::Display for ChatTemplateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ChatTemplateError {}

impl From<serde_json::Error> for ChatTemplateError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn message(role: ChatRole, content: &str) -> ChatMessage {
        ChatMessage {
            role,
            content: content.to_owned(),
            reasoning_content: None,
            tool_calls: Vec::new(),
            name: None,
            tool_call_id: None,
        }
    }

    #[test]
    fn simple_and_reasoning_prompts_match_pinned_jinja() {
        assert_eq!(
            render_chat(
                &[message(ChatRole::User, "Hello")],
                None,
                ChatTemplateOptions::default()
            )
            .unwrap(),
            "[gMASK]<sop><|system|>Reasoning Effort: Max<|user|>Hello<|assistant|><think>"
        );
        let options = ChatTemplateOptions {
            enable_thinking: false,
            ..ChatTemplateOptions::default()
        };
        assert_eq!(
            render_chat(
                &[
                    message(ChatRole::System, "Be exact."),
                    message(ChatRole::User, "Hi")
                ],
                None,
                options
            )
            .unwrap(),
            "[gMASK]<sop><|system|>Be exact.<|user|>Hi<|assistant|><think></think>"
        );
    }

    #[test]
    fn prior_reasoning_is_cleared_but_visible_content_is_preserved() {
        let mut assistant = message(ChatRole::Assistant, "<think>r</think>A");
        assistant.reasoning_content = None;
        assert_eq!(
            render_chat(
                &[
                    message(ChatRole::User, "Q"),
                    assistant,
                    message(ChatRole::User, "Next")
                ],
                None,
                ChatTemplateOptions {
                    reasoning_effort: ReasoningEffort::High,
                    ..ChatTemplateOptions::default()
                }
            )
            .unwrap(),
            "[gMASK]<sop><|system|>Reasoning Effort: High<|user|>Q<|assistant|><think></think>A<|user|>Next<|assistant|><think>"
        );
    }

    #[test]
    fn tools_retain_schema_and_argument_order() {
        let tool: OrderedValue = serde_json::from_str(
            r#"{"type":"function","function":{"name":"weather","description":"Get weather","parameters":{"type":"object","properties":{"city":{"type":"string"}}},"strict":true}}"#,
        )
        .unwrap();
        let mut assistant = message(ChatRole::Assistant, "");
        assistant.tool_calls.push(ChatToolCall {
            id: None,
            kind: None,
            function: ChatFunctionCall {
                name: "weather".to_owned(),
                arguments: serde_json::from_str(r#"{"city":"北京","days":2}"#).unwrap(),
            },
        });
        let rendered = render_chat(
            &[
                message(ChatRole::User, "Weather?"),
                assistant,
                message(ChatRole::Tool, "sunny"),
                message(ChatRole::Tool, "warm"),
            ],
            Some(&[tool]),
            ChatTemplateOptions::default(),
        )
        .unwrap();
        assert!(rendered.contains(
            r#"{"name": "weather", "description": "Get weather", "parameters": {"type": "object", "properties": {"city": {"type": "string"}}}}"#
        ));
        assert!(rendered.contains(
            "<tool_call>weather<arg_key>city</arg_key><arg_value>北京</arg_value><arg_key>days</arg_key><arg_value>2</arg_value></tool_call>"
        ));
        assert!(rendered.contains(
            "<|observation|><tool_response>sunny</tool_response><tool_response>warm</tool_response>"
        ));
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(rendered.as_bytes())),
            crate::hex_array("eabc1924f2f336acb27565e7de91f315133e8c43126291985d99a93e85384e7b")
        );
    }
}
