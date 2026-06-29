use std::collections::HashMap;

use crate::ai::AiClient;
use crate::connection::{AiConfig, DatabaseKind};

use super::tools;

#[test]
fn test_tool_definitions_returns_all_6_tools() {
    let tools = tools::tool_definitions();
    assert_eq!(tools.len(), 6);

    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(names.contains(&"riverql_query"));
    assert!(names.contains(&"riverql_describe"));
    assert!(names.contains(&"riverql_list_tables"));
    assert!(names.contains(&"riverql_explain"));
    assert!(names.contains(&"riverql_list_connections"));
    assert!(names.contains(&"riverql_help"));

    for tool in &tools {
        assert!(!tool.name.is_empty());
        assert!(tool.description.is_some());
    }
}

#[test]
fn test_resource_listing_returns_all_3_resources() {
    let resources = super::resources::all_resources();
    assert_eq!(resources.len(), 3);

    let uris: Vec<&str> = resources.iter().map(|r| r.raw.uri.as_str()).collect();
    assert!(uris.contains(&"river://docs"));
    assert!(uris.contains(&"river://docs/quickref"));
    assert!(uris.contains(&"river://docs/keywords"));

    for resource in &resources {
        assert!(!resource.raw.name.is_empty());
    }
}

#[test]
fn test_read_resource_returns_docs() {
    for uri in &[
        "river://docs",
        "river://docs/quickref",
        "river://docs/keywords",
    ] {
        let result = super::resources::read(uri);
        assert!(!result.contents.is_empty(), "no content for {}", uri);

        for content in &result.contents {
            match content {
                rmcp::model::ResourceContents::TextResourceContents { text, .. } => {
                    assert!(!text.is_empty(), "empty text for {}", uri);
                }
                _ => panic!("expected text content for {}", uri),
            }
        }
    }
}

#[test]
fn test_read_resource_unknown_uri_returns_empty() {
    let result = super::resources::read("river://nonexistent");
    assert!(result.contents.is_empty());
}

#[test]
fn test_help_overview() {
    let result = tools::handle_help(&serde_json::Map::new()).expect("help failed");
    assert!(!result.content.is_empty());
    assert_eq!(result.is_error, Some(false));
}

#[test]
fn test_help_topics() {
    for topic in &[
        "select",
        "joins",
        "aggregation",
        "window",
        "modification",
        "ddl",
        "meta",
        "cross_db",
        "operators",
        "functions",
        "keywords",
    ] {
        let mut args = serde_json::Map::new();
        args.insert(
            "topic".to_string(),
            serde_json::Value::String(topic.to_string()),
        );

        let result = tools::handle_help(&args).expect("help failed");
        assert!(
            !result.content.is_empty(),
            "empty help for topic {}",
            topic
        );
        assert_eq!(result.is_error, Some(false));
    }
}

#[test]
fn test_list_connections() {
    let source_db = vec![
        ("pg".to_string(), DatabaseKind::Postgres),
        ("mysql".to_string(), DatabaseKind::MySQL),
    ];
    let adapters: HashMap<String, Box<dyn crate::adapters::DatabaseAdapter>> = HashMap::new();

    let result =
        tools::handle_list_connections(&source_db, &adapters).expect("list_connections failed");
    assert!(!result.content.is_empty());
    assert_eq!(result.is_error, Some(false));
}

#[test]
fn test_explain_requires_query() {
    let args = serde_json::Map::new();
    let result = tools::handle_explain(&args);
    assert!(result.is_err());
}

#[test]
fn test_tool_dispatch_unknown_tool() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let adapters: HashMap<String, Box<dyn crate::adapters::DatabaseAdapter>> = HashMap::new();
    let source_db: Vec<(String, DatabaseKind)> = vec![];
    let ai_configs: HashMap<String, AiConfig> = HashMap::new();
    let ai_client = AiClient::new();

    let result = rt
        .block_on(tools::dispatch(&adapters, &source_db, &ai_configs, &ai_client, "nonexistent_tool", None))
        .expect("dispatch should succeed");

    assert_eq!(result.is_error, Some(true));
}

#[test]
fn test_tool_dispatch_parse_error() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let adapters: HashMap<String, Box<dyn crate::adapters::DatabaseAdapter>> = HashMap::new();
    let source_db: Vec<(String, DatabaseKind)> = vec![];
    let ai_configs: HashMap<String, AiConfig> = HashMap::new();
    let ai_client = AiClient::new();

    let mut args = serde_json::Map::new();
    args.insert(
        "query".to_string(),
        serde_json::Value::String("invalid $$$ query".to_string()),
    );

    let result = rt.block_on(tools::dispatch(
        &adapters,
        &source_db,
        &ai_configs,
        &ai_client,
        "riverql_query",
        Some(args),
    ));

    match result {
        Err(e) => assert!(!e.message.is_empty()),
        Ok(_) => panic!("expected parse error, got successful result"),
    }
}
