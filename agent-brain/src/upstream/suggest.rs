use crate::db::store::BrainStore;
use crate::settings::UpstreamMcpSettings;
use crate::types::SuggestedTool;
use crate::upstream::{enabled_servers, IndexedUpstreamTool};

pub fn suggest_upstream_tools(
    store: &BrainStore,
    settings: &UpstreamMcpSettings,
    user_message: &str,
    limit: usize,
) -> Vec<SuggestedTool> {
    if !settings.enabled || limit == 0 {
        return Vec::new();
    }
    let Ok(tools) = store.list_upstream_tools() else {
        return Vec::new();
    };
    if tools.is_empty() {
        return Vec::new();
    }

    let allowed: std::collections::HashSet<String> = enabled_servers(settings)
        .into_iter()
        .map(|s| s.name.to_ascii_lowercase())
        .collect();

    let query = user_message.to_ascii_lowercase();
    let query_tokens: Vec<&str> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 3)
        .collect();

    let mut scored: Vec<(f64, &IndexedUpstreamTool)> = tools
        .iter()
        .filter(|tool| allowed.contains(&tool.server.to_ascii_lowercase()))
        .map(|tool| (score_tool(&query, &query_tokens, tool), tool))
        .filter(|(score, _)| *score > 0.0)
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    scored
        .into_iter()
        .take(limit)
        .map(|(score, tool)| SuggestedTool {
            server: tool.server.clone(),
            tool: tool.name.clone(),
            description: tool.description.clone(),
            rationale: format!("keyword match for {}", tool.name),
            score,
        })
        .collect()
}

fn score_tool(query: &str, query_tokens: &[&str], tool: &IndexedUpstreamTool) -> f64 {
    let haystack = format!(
        "{} {} {}",
        tool.server.to_ascii_lowercase(),
        tool.name.to_ascii_lowercase(),
        tool.description.to_ascii_lowercase()
    );
    let mut score = 0.0;
    for token in query_tokens {
        if haystack.contains(token) {
            score += 1.0;
        }
    }
    if query.contains(&tool.name.to_ascii_lowercase()) {
        score += 2.0;
    }
    if query.contains(&tool.server.to_ascii_lowercase()) {
        score += 1.5;
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upstream::IndexedUpstreamTool;

    #[test]
    fn scores_github_issue_lookup() {
        let tools = vec![IndexedUpstreamTool {
            server: "github".into(),
            name: "search_issues".into(),
            description: "Search GitHub issues in a repository".into(),
        }];
        let query = "find open github issues in agent-brain repo";
        let query_lower = query.to_ascii_lowercase();
        let query_tokens: Vec<&str> = query_lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| t.len() >= 3)
            .collect();
        let score = score_tool(&query_lower, &query_tokens, &tools[0]);
        assert!(score >= 2.0);
    }
}
