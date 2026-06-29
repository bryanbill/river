use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use rmcp::{
    ErrorData as McpError, ServiceExt,
    model::{
        CallToolRequestParam, CallToolResult, Implementation, ListResourcesResult,
        ListToolsResult, PaginatedRequestParam, ReadResourceRequestParam, ReadResourceResult,
        ServerCapabilities, ServerInfo,
    },
    service::{RequestContext, RoleServer},
};
use tracing::info;

use crate::adapters::DatabaseAdapter;
use crate::ai::AiClient;
use crate::connection::{AiConfig, DatabaseKind};

use super::{resources, tools};

#[derive(Clone)]
pub struct RiverServer {
    adapters: Arc<HashMap<String, Box<dyn DatabaseAdapter>>>,
    source_db: Arc<Vec<(String, DatabaseKind)>>,
    ai_configs: Arc<HashMap<String, AiConfig>>,
    ai_client: Arc<AiClient>,
}

impl RiverServer {
    pub fn new(
        adapters: HashMap<String, Box<dyn DatabaseAdapter>>,
        source_db: Vec<(String, DatabaseKind)>,
        ai_configs: HashMap<String, AiConfig>,
        ai_client: AiClient,
    ) -> Self {
        Self {
            adapters: Arc::new(adapters),
            source_db: Arc::new(source_db),
            ai_configs: Arc::new(ai_configs),
            ai_client: Arc::new(ai_client),
        }
    }
}

impl rmcp::ServerHandler for RiverServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: Default::default(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
            server_info: Implementation {
                name: "River".into(),
                title: Some("River — Unified Database Access".into()),
                version: env!("CARGO_PKG_VERSION").into(),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "River MCP Server — Unified database access via RiverQL.\n\
                 Use riverql_help to learn the query language.\n\
                 Use riverql_query to execute queries across PostgreSQL, MySQL, SQLite, MongoDB, and MSSQL."
                    .into(),
            ),
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        let adapters = self.adapters.clone();
        let source_db = self.source_db.clone();
        let ai_configs = self.ai_configs.clone();
        let ai_client = self.ai_client.clone();
        async move {
            info!("tool call: {} with args: {:?}", request.name, request.arguments);
            tools::dispatch(&adapters, &source_db, &ai_configs, &ai_client, &request.name, request.arguments).await
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        std::future::ready(Ok(ListToolsResult {
            tools: tools::tool_definitions(),
            next_cursor: None,
        }))
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, McpError>> + Send + '_ {
        std::future::ready(Ok(ListResourcesResult {
            resources: resources::all_resources(),
            next_cursor: None,
        }))
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ReadResourceResult, McpError>> + Send + '_ {
        std::future::ready(Ok(resources::read(&request.uri)))
    }
}

pub async fn run_mcp_server(
    adapters: HashMap<String, Box<dyn DatabaseAdapter>>,
    source_db: Vec<(String, DatabaseKind)>,
    ai_configs: HashMap<String, AiConfig>,
    ai_client: AiClient,
) -> anyhow::Result<()> {
    info!(
        "Starting MCP server with {} connections",
        source_db.len()
    );

    let server = RiverServer::new(adapters, source_db, ai_configs, ai_client);

    let transport = rmcp::transport::io::stdio();

    let running = server.serve(transport).await?;

    running.waiting().await?;

    Ok(())
}
