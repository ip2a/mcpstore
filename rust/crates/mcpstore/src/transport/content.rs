use crate::transport::{ContentItem, Result, TransportError};
use rmcp::model::ContentBlock;

fn optional_json<T: serde::Serialize>(value: Option<T>) -> Option<serde_json::Value> {
    value.and_then(|value| serde_json::to_value(value).ok())
}

pub(in crate::transport) fn content_item_from_rmcp(content: ContentBlock) -> Result<ContentItem> {
    match content {
        ContentBlock::Text(text) => Ok(ContentItem::Text {
            text: text.text,
            annotations: optional_json(text.annotations),
            meta: optional_json(text.meta),
        }),
        ContentBlock::Image(image) => Ok(ContentItem::Image {
            data: image.data,
            mime_type: image.mime_type,
            annotations: optional_json(image.annotations),
            meta: optional_json(image.meta),
        }),
        ContentBlock::Audio(audio) => Ok(ContentItem::Audio {
            data: audio.data,
            mime_type: audio.mime_type,
            annotations: optional_json(audio.annotations),
            meta: optional_json(audio.meta),
        }),
        ContentBlock::Resource(resource) => Ok(ContentItem::Resource {
            resource: serde_json::to_value(resource.resource).unwrap_or(serde_json::Value::Null),
            annotations: optional_json(resource.annotations),
            meta: optional_json(resource.meta),
        }),
        ContentBlock::ResourceLink(mut resource) => {
            let annotations = optional_json(resource.annotations.take());
            Ok(ContentItem::ResourceLink {
                resource: serde_json::to_value(resource).unwrap_or(serde_json::Value::Null),
                annotations,
            })
        }
        _ => Err(TransportError::Protocol(
            "rmcp returned an unsupported content block".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::{ContentBlock, Resource, ResourceContents};

    #[test]
    fn converts_non_text_tool_content_without_downgrading() {
        let image = content_item_from_rmcp(ContentBlock::image("image-data", "image/png"))
            .expect("image content should convert");
        match image {
            ContentItem::Image {
                data, mime_type, ..
            } => {
                assert_eq!(data, "image-data");
                assert_eq!(mime_type, "image/png");
            }
            other => panic!("expected image content, got {other:?}"),
        }

        let audio = content_item_from_rmcp(ContentBlock::audio("audio-data", "audio/wav"))
            .expect("audio content should convert");
        match audio {
            ContentItem::Audio {
                data, mime_type, ..
            } => {
                assert_eq!(data, "audio-data");
                assert_eq!(mime_type, "audio/wav");
            }
            other => panic!("expected audio content, got {other:?}"),
        }

        let resource = content_item_from_rmcp(ContentBlock::resource(ResourceContents::text(
            "body",
            "memory://doc",
        )))
        .expect("embedded resource content should convert");
        match resource {
            ContentItem::Resource { resource, .. } => {
                assert_eq!(resource["uri"], "memory://doc");
                assert_eq!(resource["text"], "body");
            }
            other => panic!("expected resource content, got {other:?}"),
        }

        let resource_link = content_item_from_rmcp(ContentBlock::resource_link(
            Resource::new("file:///tmp/demo.txt", "demo.txt").with_mime_type("text/plain"),
        ))
        .expect("resource link content should convert");
        match resource_link {
            ContentItem::ResourceLink { resource, .. } => {
                assert_eq!(resource["uri"], "file:///tmp/demo.txt");
                assert_eq!(resource["name"], "demo.txt");
            }
            other => panic!("expected resource link content, got {other:?}"),
        }
    }
}
