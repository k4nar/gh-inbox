use serde::Deserialize;

use super::GithubClient;

#[derive(Debug, Deserialize)]
struct GithubTeam {
    slug: String,
    organization: GithubOrg,
}

#[derive(Debug, Deserialize)]
struct GithubOrg {
    login: String,
}

/// Fetch the teams the authenticated user belongs to.
/// Returns full slugs in "{org}/{team}" format.
///
/// Note: only the first page (up to 100 teams) is fetched. Users belonging to
/// more than 100 teams across all organisations will silently miss the overflow.
/// Pagination support can be added later if this becomes a real-world problem.
pub async fn fetch_user_teams(github: &GithubClient) -> Result<Vec<String>, reqwest::Error> {
    let teams: Vec<GithubTeam> = github
        .get("/user/teams?per_page=100")
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(teams
        .into_iter()
        .map(|t| format!("{}/{}", t.organization.login, t.slug))
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
}
