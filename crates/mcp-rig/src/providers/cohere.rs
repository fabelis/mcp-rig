//! Cohere API client and Rig integration
//!
//! # Example
//! ```
//! use mcp_rig::providers::cohere;
//!
//! let client = cohere::Client::new("YOUR_API_KEY");
//!
//! let command_r = client.completion_model(cohere::COMMAND_R);
//! ```
use std::collections::HashMap;

use crate::{
    agent::AgentBuilder,
    completion::{self, CompletionError},
    embeddings::{self, EmbeddingError, EmbeddingsBuilder},
    extractor::ExtractorBuilder,
    json_utils, message, Embed, OneOrMany,
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

// ================================================================
// Main Cohere Client
// ================================================================
const COHERE_API_BASE_URL: &str = "https://api.cohere.ai";

#[derive(Clone)]
pub struct Client {
    base_url: String,
    http_client: reqwest::Client,
}

impl Client {
    pub fn new(api_key: &str) -> Self {
        Self::from_url(api_key, COHERE_API_BASE_URL)
    }

    pub fn from_url(api_key: &str, base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            http_client: reqwest::Client::builder()
                .default_headers({
                    let mut headers = reqwest::header::HeaderMap::new();
                    headers.insert(
                        "Authorization",
                        format!("Bearer {}", api_key)
                            .parse()
                            .expect("Bearer token should parse"),
                    );
                    headers
                })
                .build()
                .expect("Cohere reqwest client should build"),
        }
    }

    /// Create a new Cohere client from the `COHERE_API_KEY` environment variable.
    /// Panics if the environment variable is not set.
    pub fn from_env() -> Self {
        let api_key = std::env::var("COHERE_API_KEY").expect("COHERE_API_KEY not set");
        Self::new(&api_key)
    }

    pub fn post(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}/{}", self.base_url, path).replace("//", "/");
        self.http_client.post(url)
    }

    /// Note: default embedding dimension of 0 will be used if model is not known.
    /// If this is the case, it's better to use function `embedding_model_with_ndims`
    pub fn embedding_model(&self, model: &str, input_type: &str) -> EmbeddingModel {
        let ndims = match model {
            EMBED_ENGLISH_V3 | EMBED_MULTILINGUAL_V3 | EMBED_ENGLISH_LIGHT_V2 => 1024,
            EMBED_ENGLISH_LIGHT_V3 | EMBED_MULTILINGUAL_LIGHT_V3 => 384,
            EMBED_ENGLISH_V2 => 4096,
            EMBED_MULTILINGUAL_V2 => 768,
            _ => 0,
        };
        EmbeddingModel::new(self.clone(), model, input_type, ndims)
    }

    /// Create an embedding model with the given name and the number of dimensions in the embedding generated by the model.
    pub fn embedding_model_with_ndims(
        &self,
        model: &str,
        input_type: &str,
        ndims: usize,
    ) -> EmbeddingModel {
        EmbeddingModel::new(self.clone(), model, input_type, ndims)
    }

    pub fn embeddings<D: Embed>(
        &self,
        model: &str,
        input_type: &str,
    ) -> EmbeddingsBuilder<EmbeddingModel, D> {
        EmbeddingsBuilder::new(self.embedding_model(model, input_type))
    }

    pub fn completion_model(&self, model: &str) -> CompletionModel {
        CompletionModel::new(self.clone(), model)
    }

    pub fn agent(&self, model: &str) -> AgentBuilder<CompletionModel> {
        AgentBuilder::new(self.completion_model(model))
    }

    pub fn extractor<T: JsonSchema + for<'a> Deserialize<'a> + Serialize + Send + Sync>(
        &self,
        model: &str,
    ) -> ExtractorBuilder<T, CompletionModel> {
        ExtractorBuilder::new(self.completion_model(model))
    }
}

#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ApiResponse<T> {
    Ok(T),
    Err(ApiErrorResponse),
}

// ================================================================
// Cohere Embedding API
// ================================================================
/// `embed-english-v3.0` embedding model
pub const EMBED_ENGLISH_V3: &str = "embed-english-v3.0";
/// `embed-english-light-v3.0` embedding model
pub const EMBED_ENGLISH_LIGHT_V3: &str = "embed-english-light-v3.0";
/// `embed-multilingual-v3.0` embedding model
pub const EMBED_MULTILINGUAL_V3: &str = "embed-multilingual-v3.0";
/// `embed-multilingual-light-v3.0` embedding model
pub const EMBED_MULTILINGUAL_LIGHT_V3: &str = "embed-multilingual-light-v3.0";
/// `embed-english-v2.0` embedding model
pub const EMBED_ENGLISH_V2: &str = "embed-english-v2.0";
/// `embed-english-light-v2.0` embedding model
pub const EMBED_ENGLISH_LIGHT_V2: &str = "embed-english-light-v2.0";
/// `embed-multilingual-v2.0` embedding model
pub const EMBED_MULTILINGUAL_V2: &str = "embed-multilingual-v2.0";

#[derive(Deserialize)]
pub struct EmbeddingResponse {
    #[serde(default)]
    pub response_type: Option<String>,
    pub id: String,
    pub embeddings: Vec<Vec<f64>>,
    pub texts: Vec<String>,
    #[serde(default)]
    pub meta: Option<Meta>,
}

#[derive(Deserialize)]
pub struct Meta {
    pub api_version: ApiVersion,
    pub billed_units: BilledUnits,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Deserialize)]
pub struct ApiVersion {
    pub version: String,
    #[serde(default)]
    pub is_deprecated: Option<bool>,
    #[serde(default)]
    pub is_experimental: Option<bool>,
}

#[derive(Deserialize, Debug)]
pub struct BilledUnits {
    #[serde(default)]
    pub input_tokens: u32,
    #[serde(default)]
    pub output_tokens: u32,
    #[serde(default)]
    pub search_units: u32,
    #[serde(default)]
    pub classifications: u32,
}

impl std::fmt::Display for BilledUnits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Input tokens: {}\nOutput tokens: {}\nSearch units: {}\nClassifications: {}",
            self.input_tokens, self.output_tokens, self.search_units, self.classifications
        )
    }
}

#[derive(Clone)]
pub struct EmbeddingModel {
    client: Client,
    pub model: String,
    pub input_type: String,
    ndims: usize,
}

impl embeddings::EmbeddingModel for EmbeddingModel {
    const MAX_DOCUMENTS: usize = 96;

    fn ndims(&self) -> usize {
        self.ndims
    }

    #[cfg_attr(feature = "worker", worker::send)]
    async fn embed_texts(
        &self,
        documents: impl IntoIterator<Item = String>,
    ) -> Result<Vec<embeddings::Embedding>, EmbeddingError> {
        let documents = documents.into_iter().collect::<Vec<_>>();

        let response = self
            .client
            .post("/v1/embed")
            .json(&json!({
                "model": self.model,
                "texts": documents,
                "input_type": self.input_type,
            }))
            .send()
            .await?;

        if response.status().is_success() {
            match response.json::<ApiResponse<EmbeddingResponse>>().await? {
                ApiResponse::Ok(response) => {
                    match response.meta {
                        Some(meta) => tracing::info!(target: "rig",
                            "Cohere embeddings billed units: {}",
                            meta.billed_units,
                        ),
                        None => tracing::info!(target: "rig",
                            "Cohere embeddings billed units: n/a",
                        ),
                    };

                    if response.embeddings.len() != documents.len() {
                        return Err(EmbeddingError::DocumentError(
                            format!(
                                "Expected {} embeddings, got {}",
                                documents.len(),
                                response.embeddings.len()
                            )
                            .into(),
                        ));
                    }

                    Ok(response
                        .embeddings
                        .into_iter()
                        .zip(documents.into_iter())
                        .map(|(embedding, document)| embeddings::Embedding {
                            document,
                            vec: embedding,
                        })
                        .collect())
                }
                ApiResponse::Err(error) => Err(EmbeddingError::ProviderError(error.message)),
            }
        } else {
            Err(EmbeddingError::ProviderError(response.text().await?))
        }
    }
}

impl EmbeddingModel {
    pub fn new(client: Client, model: &str, input_type: &str, ndims: usize) -> Self {
        Self {
            client,
            model: model.to_string(),
            input_type: input_type.to_string(),
            ndims,
        }
    }
}

// ================================================================
// Cohere Completion API
// ================================================================
/// `command-r-plus` completion model
pub const COMMAND_R_PLUS: &str = "comman-r-plus";
/// `command-r` completion model
pub const COMMAND_R: &str = "command-r";
/// `command` completion model
pub const COMMAND: &str = "command";
/// `command-nightly` completion model
pub const COMMAND_NIGHTLY: &str = "command-nightly";
/// `command-light` completion model
pub const COMMAND_LIGHT: &str = "command-light";
/// `command-light-nightly` completion model
pub const COMMAND_LIGHT_NIGHTLY: &str = "command-light-nightly";

#[derive(Debug, Deserialize)]
pub struct CompletionResponse {
    pub text: String,
    pub generation_id: String,
    #[serde(default)]
    pub citations: Vec<Citation>,
    #[serde(default)]
    pub documents: Vec<Document>,
    #[serde(default)]
    pub is_search_required: Option<bool>,
    #[serde(default)]
    pub search_queries: Vec<SearchQuery>,
    #[serde(default)]
    pub search_results: Vec<SearchResult>,
    pub finish_reason: String,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default)]
    pub chat_history: Vec<ChatHistory>,
}

impl From<CompletionResponse> for completion::CompletionResponse<CompletionResponse> {
    fn from(response: CompletionResponse) -> Self {
        let CompletionResponse {
            text, tool_calls, ..
        } = &response;

        let model_response = if !tool_calls.is_empty() {
            tool_calls
                .iter()
                .map(|tool_call| {
                    completion::AssistantContent::tool_call(
                        tool_call.name.clone(),
                        tool_call.name.clone(),
                        tool_call.parameters.clone(),
                    )
                })
                .collect::<Vec<_>>()
        } else {
            vec![completion::AssistantContent::text(text.clone())]
        };

        completion::CompletionResponse {
            choice: OneOrMany::many(model_response).expect("There is atleast one content"),
            raw_response: response,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Citation {
    pub start: u32,
    pub end: u32,
    pub text: String,
    pub document_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Document {
    pub id: String,
    #[serde(flatten)]
    pub additional_prop: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub text: String,
    pub generation_id: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchResult {
    pub search_query: SearchQuery,
    pub connector: Connector,
    pub document_ids: Vec<String>,
    #[serde(default)]
    pub error_message: Option<String>,
    #[serde(default)]
    pub continue_on_failure: bool,
}

#[derive(Debug, Deserialize)]
pub struct Connector {
    pub id: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ToolCall {
    pub name: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct ChatHistory {
    pub role: String,
    pub message: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Parameter {
    pub description: String,
    pub r#type: String,
    pub required: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameter_definitions: HashMap<String, Parameter>,
}

impl From<completion::ToolDefinition> for ToolDefinition {
    fn from(tool: completion::ToolDefinition) -> Self {
        fn convert_type(r#type: &serde_json::Value) -> String {
            fn convert_type_str(r#type: &str) -> String {
                match r#type {
                    "string" => "string".to_owned(),
                    "number" => "number".to_owned(),
                    "integer" => "integer".to_owned(),
                    "boolean" => "boolean".to_owned(),
                    "array" => "array".to_owned(),
                    "object" => "object".to_owned(),
                    _ => "string".to_owned(),
                }
            }
            match r#type {
                serde_json::Value::String(r#type) => convert_type_str(r#type.as_str()),
                serde_json::Value::Array(types) => convert_type_str(
                    types
                        .iter()
                        .find(|t| t.as_str() != Some("null"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("string"),
                ),
                _ => "string".to_owned(),
            }
        }

        let maybe_required = tool
            .parameters
            .get("required")
            .and_then(|v| v.as_array())
            .map(|required| {
                required
                    .iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Self {
            name: tool.name,
            description: tool.description,
            parameter_definitions: tool
                .parameters
                .get("properties")
                .expect("Tool properties should exist")
                .as_object()
                .expect("Tool properties should be an object")
                .iter()
                .map(|(argname, argdef)| {
                    (
                        argname.clone(),
                        Parameter {
                            description: argdef
                                .get("description")
                                .expect("Argument description should exist")
                                .as_str()
                                .expect("Argument description should be a string")
                                .to_string(),
                            r#type: convert_type(
                                argdef.get("type").expect("Argument type should exist"),
                            ),
                            required: maybe_required.contains(&argname.as_str()),
                        },
                    )
                })
                .collect::<HashMap<_, _>>(),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "role", rename_all = "UPPERCASE")]
pub enum Message {
    User {
        message: String,
        tool_calls: Vec<ToolCall>,
    },

    Chatbot {
        message: String,
        tool_calls: Vec<ToolCall>,
    },

    Tool {
        tool_results: Vec<ToolResult>,
    },

    /// According to the documentation, this message type should not be used
    System {
        content: String,
        tool_calls: Vec<ToolCall>,
    },
}

#[derive(Deserialize, Serialize)]
pub struct ToolResult {
    pub call: ToolCall,
    pub outputs: Vec<serde_json::Value>,
}

impl TryFrom<message::Message> for Vec<Message> {
    type Error = message::MessageError;

    fn try_from(message: message::Message) -> Result<Self, Self::Error> {
        match message {
            message::Message::User { content } => content
                .into_iter()
                .map(|content| {
                    Ok(Message::User {
                        message: match content {
                            message::UserContent::Text(message::Text { text }) => text,
                            _ => {
                                return Err(message::MessageError::ConversionError(
                                    "Only text content is supported by Cohere".to_owned(),
                                ))
                            }
                        },
                        tool_calls: vec![],
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            _ => Err(message::MessageError::ConversionError(
                "Only user messages are supported by Cohere".to_owned(),
            )),
        }
    }
}

#[derive(Clone)]
pub struct CompletionModel {
    client: Client,
    pub model: String,
}

impl CompletionModel {
    pub fn new(client: Client, model: &str) -> Self {
        Self {
            client,
            model: model.to_string(),
        }
    }
}

impl completion::CompletionModel for CompletionModel {
    type Response = CompletionResponse;

    #[cfg_attr(feature = "worker", worker::send)]
    async fn completion(
        &self,
        completion_request: completion::CompletionRequest,
    ) -> Result<completion::CompletionResponse<CompletionResponse>, CompletionError> {
        let chat_history = completion_request
            .chat_history
            .into_iter()
            .map(Vec::<Message>::try_from)
            .collect::<Result<Vec<Vec<_>>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        let message = match completion_request.prompt {
            message::Message::User { content } => Ok(content
                .into_iter()
                .map(|content| match content {
                    message::UserContent::Text(message::Text { text }) => Ok(text),
                    _ => Err(CompletionError::RequestError(
                        "Only text content is supported by Cohere".into(),
                    )),
                })
                .collect::<Result<Vec<_>, _>>()?
                .join("\n")),

            _ => Err(CompletionError::RequestError(
                "Only user messages are supported by Cohere".into(),
            )),
        }?;

        let request = json!({
            "model": self.model,
            "preamble": completion_request.preamble,
            "message": message,
            "documents": completion_request.documents,
            "chat_history": chat_history,
            "temperature": completion_request.temperature,
            "tools": completion_request.tools.into_iter().map(ToolDefinition::from).collect::<Vec<_>>(),
        });

        let response = self
            .client
            .post("/v1/chat")
            .json(
                &if let Some(ref params) = completion_request.additional_params {
                    json_utils::merge(request.clone(), params.clone())
                } else {
                    request.clone()
                },
            )
            .send()
            .await?;

        if response.status().is_success() {
            match response.json::<ApiResponse<CompletionResponse>>().await? {
                ApiResponse::Ok(completion) => Ok(completion.into()),
                ApiResponse::Err(error) => Err(CompletionError::ProviderError(error.message)),
            }
        } else {
            Err(CompletionError::ProviderError(response.text().await?))
        }
    }
}
