use crate::transport::ContentItem;

fn optional_json<T: serde::Serialize>(value: Option<T>) -> Option<serde_json::Value> {
    value.and_then(|value| serde_json::to_value(value).ok())
}

pub(in crate::transport) fn content_item_from_rmcp(content: rmcp::model::Content) -> ContentItem {
    let annotations = optional_json(content.annotations);
    match content.raw {
        rmcp::model::RawContent::Text(text) => ContentItem::Text {
            text: text.text,
            annotations,
            meta: optional_json(text.meta),
        },
        rmcp::model::RawContent::Image(image) => ContentItem::Image {
            data: image.data,
            mime_type: image.mime_type,
            annotations,
            meta: optional_json(image.meta),
        },
        rmcp::model::RawContent::Audio(audio) => ContentItem::Audio {
            data: audio.data,
            mime_type: audio.mime_type,
            annotations,
        },
        rmcp::model::RawContent::Resource(resource) => ContentItem::Resource {
            resource: serde_json::to_value(resource.resource).unwrap_or(serde_json::Value::Null),
            annotations,
            meta: optional_json(resource.meta),
        },
        rmcp::model::RawContent::ResourceLink(resource) => ContentItem::ResourceLink {
            resource: serde_json::to_value(resource).unwrap_or(serde_json::Value::Null),
            annotations,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::{AnnotateAble, RawAudioContent, RawContent, RawResource, ResourceContents};

    #[test]
    fn converts_non_text_tool_content_without_downgrading() {
        let image =
            content_item_from_rmcp(RawContent::image("image-data", "image/png").no_annotation());
        match image {
            ContentItem::Image {
                data, mime_type, ..
            } => {
                assert_eq!(data, "image-data");
                assert_eq!(mime_type, "image/png");
            }
            other => panic!("expected image content, got {other:?}"),
        }

        let audio = content_item_from_rmcp(
            RawContent::Audio(RawAudioContent {
                data: "audio-data".to_string(),
                mime_type: "audio/wav".to_string(),
            })
            .no_annotation(),
        );
        match audio {
            ContentItem::Audio {
                data, mime_type, ..
            } => {
                assert_eq!(data, "audio-data");
                assert_eq!(mime_type, "audio/wav");
            }
            other => panic!("expected audio content, got {other:?}"),
        }

        let resource = content_item_from_rmcp(
            RawContent::resource(ResourceContents::text("body", "memory://doc")).no_annotation(),
        );
        match resource {
            ContentItem::Resource { resource, .. } => {
                assert_eq!(resource["uri"], "memory://doc");
                assert_eq!(resource["text"], "body");
            }
            other => panic!("expected resource content, got {other:?}"),
        }

        let resource_link = content_item_from_rmcp(
            RawContent::resource_link(RawResource {
                uri: "file:///tmp/demo.txt".to_string(),
                name: "demo.txt".to_string(),
                title: None,
                description: None,
                mime_type: Some("text/plain".to_string()),
                size: None,
                icons: None,
                meta: None,
            })
            .no_annotation(),
        );
        match resource_link {
            ContentItem::ResourceLink { resource, .. } => {
                assert_eq!(resource["uri"], "file:///tmp/demo.txt");
                assert_eq!(resource["name"], "demo.txt");
            }
            other => panic!("expected resource link content, got {other:?}"),
        }
    }
}
