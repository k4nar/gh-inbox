use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GithubTeam {
    slug: String,
    organization: GithubOrg,
}

#[derive(Debug, Deserialize)]
struct GithubOrg {
    login: String,
}

#[derive(Debug, Deserialize)]
struct ReviewersResponse {
    teams: Vec<ReviewerTeam>,
}

#[derive(Debug, Deserialize)]
struct ReviewerTeam {
    slug: String,
    // org is not in the reviewer response; we derive it from the repo owner
}

/// Fetch the teams the authenticated user belongs to.
/// Returns full slugs in "{org}/{team}" format.
///
/// Note: only the first page (up to 100 teams) is fetched. Users belonging to
/// more than 100 teams across all organisations will silently miss the overflow.
/// Pagination support can be added later if this becomes a real-world problem.
pub async fn fetch_user_teams(
    client: &reqwest::Client,
    token: &str,
    base_url: &str,
) -> Result<Vec<String>, reqwest::Error> {
    let url = format!("{base_url}/user/teams?per_page=100");
    let teams: Vec<GithubTeam> = super::github_request(client, token, &url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(teams
        .into_iter()
        .map(|t| format!("{}/{}", t.organization.login, t.slug))
        .collect())
}

/// Fetch the teams requested to review a specific PR.
/// Returns team slugs as "{owner}/{team}" (owner = repo org login).
pub async fn fetch_requested_reviewer_teams(
    client: &reqwest::Client,
    token: &str,
    base_url: &str,
    owner: &str,
    repo: &str,
    pr_number: i64,
) -> Result<Vec<String>, reqwest::Error> {
    let url = format!("{base_url}/repos/{owner}/{repo}/pulls/{pr_number}/requested_reviewers");
    let response: ReviewersResponse = super::github_request(client, token, &url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(response
        .teams
        .into_iter()
        .map(|t| format!("{owner}/{}", t.slug))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_user_teams(json: &str) -> Vec<String> {
        let teams: Vec<GithubTeam> = serde_json::from_str(json).unwrap();
        teams
            .into_iter()
            .map(|t| format!("{}/{}", t.organization.login, t.slug))
            .collect()
    }

    fn parse_reviewer_teams(json: &str, owner: &str) -> Vec<String> {
        let response: ReviewersResponse = serde_json::from_str(json).unwrap();
        response
            .teams
            .into_iter()
            .map(|t| format!("{owner}/{}", t.slug))
            .collect()
    }

    #[test]
    fn parse_user_teams_valid() {
        let json = r#"[
            {"slug": "platform", "organization": {"login": "acme"}},
            {"slug": "backend", "organization": {"login": "acme"}}
        ]"#;
        let result = parse_user_teams(json);
        assert_eq!(result, vec!["acme/platform", "acme/backend"]);
    }

    #[test]
    fn parse_user_teams_empty() {
        let result = parse_user_teams("[]");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_reviewer_teams_valid() {
        let json = r#"{"users": [], "teams": [{"slug": "platform"}, {"slug": "infra"}]}"#;
        let result = parse_reviewer_teams(json, "acme");
        assert_eq!(result, vec!["acme/platform", "acme/infra"]);
    }

    #[test]
    fn parse_reviewer_teams_no_teams() {
        let json = r#"{"users": [{"login": "alice"}], "teams": []}"#;
        let result = parse_reviewer_teams(json, "acme");
        assert!(result.is_empty());
    }
}
