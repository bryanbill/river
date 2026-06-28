use rmcp::model::{RawResource, Resource, ResourceContents, ReadResourceResult};

use super::docs;

pub fn all_resources() -> Vec<Resource> {
    vec![
        Resource {
            raw: RawResource {
                uri: "river://docs".into(),
                name: "RiverQL Full Reference".into(),
                title: None,
                description: Some("Complete RiverQL language reference documentation.".into()),
                mime_type: Some("text/markdown".into()),
                size: Some(docs::full_reference().len() as u32),
                icons: None,
            },
            annotations: None,
        },
        Resource {
            raw: RawResource {
                uri: "river://docs/quickref".into(),
                name: "RiverQL Quick Reference".into(),
                title: None,
                description: Some(
                    "Concise quick reference: keywords, operators, functions, and cardinal rules."
                        .into(),
                ),
                mime_type: Some("text/markdown".into()),
                size: Some(docs::quickref().len() as u32),
                icons: None,
            },
            annotations: None,
        },
        Resource {
            raw: RawResource {
                uri: "river://docs/keywords".into(),
                name: "RiverQL Keywords".into(),
                title: None,
                description: Some("Keyword-to-purpose mapping table.".into()),
                mime_type: Some("text/markdown".into()),
                size: Some(docs::keywords().len() as u32),
                icons: None,
            },
            annotations: None,
        },
    ]
}

pub fn read(uri: &str) -> ReadResourceResult {
    let text = match uri {
        "river://docs" => docs::full_reference(),
        "river://docs/quickref" => docs::quickref(),
        "river://docs/keywords" => docs::keywords(),
        _ => return ReadResourceResult { contents: vec![] },
    };

    ReadResourceResult {
        contents: vec![ResourceContents::TextResourceContents {
            uri: uri.to_string(),
            mime_type: Some("text/markdown".to_string()),
            text: text.to_string(),
            meta: None,
        }],
    }
}
