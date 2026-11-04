use crate::config::ScopesConfig;

#[test]
fn test_parse_scopes_toml() {
    let toml_content = r#"
[store.services.github]
enabled = true

[store.services.github.tools]
policy = ["search_repos", "get_repo"]

[store.services.github.tools.overrides.search_repos]
display_name = "🔍 Search"
tags = ["search"]

[store.services.github.tools.overrides.search_repos.arguments.query]
default = "test"

[agents.admin.services.github.tools]
policy = ["search_repos", "create_issue"]
"#;

    let config = ScopesConfig::parse_toml(toml_content).expect("Failed to parse");

    // Store scope checks
    assert!(config.store.services.contains_key("github"));
    let github = &config.store.services["github"];
    assert_eq!(github.enabled, Some(true));
    assert_eq!(github.tools.policy, Some(vec!["search_repos".to_string(), "get_repo".to_string()]));

    let search_override = &github.tools.overrides["search_repos"];
    assert_eq!(search_override.display_name, Some("🔍 Search".to_string()));
    assert_eq!(search_override.tags, vec!["search".to_string()]);

    let query_arg = &search_override.arguments.as_ref().unwrap()["query"];
    assert_eq!(query_arg.default, Some(serde_json::json!("test")));

    // Agent scope checks
    assert!(config.agents.contains_key("admin"));
    let admin_github = &config.agents["admin"].services["github"];
    assert_eq!(admin_github.tools.policy, Some(vec!["search_repos".to_string(), "create_issue".to_string()]));
}

#[test]
fn test_scopes_config_empty() {
    let config = ScopesConfig::default();
    assert!(config.store.services.is_empty());
    assert!(config.agents.is_empty());
}
