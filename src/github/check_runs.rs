use crate::models::GithubCheckRunList;

pub async fn fetch_check_runs(
    token: &str,
    client: &reqwest::Client,
    base_url: &str,
    owner: &str,
    repo: &str,
    sha: &str,
) -> Result<GithubCheckRunList, reqwest::Error> {
    let url = format!("{base_url}/repos/{owner}/{repo}/commits/{sha}/check-runs");
    super::send_github_request(super::github_request(client, token, &url), "GET", &url)
        .await?
        .error_for_status()?
        .json()
        .await
}

#[cfg(test)]
mod tests {
    use crate::models::GithubCheckRunList;

    fn parse_check_runs(json: &str) -> Result<GithubCheckRunList, serde_json::Error> {
        serde_json::from_str(json)
    }

    #[test]
    fn parse_valid_check_runs() {
        let json = r#"{
            "total_count": 2,
            "check_runs": [
                { "id": 1, "name": "CI", "status": "completed", "conclusion": "success" },
                { "id": 2, "name": "Lint", "status": "in_progress", "conclusion": null }
            ]
        }"#;
        let result = parse_check_runs(json).unwrap();
        assert_eq!(result.total_count, 2);
        assert_eq!(result.check_runs.len(), 2);
        assert_eq!(result.check_runs[0].conclusion, Some("success".to_string()));
        assert!(result.check_runs[1].conclusion.is_none());
    }

    #[test]
    fn parse_empty_check_runs() {
        let json = r#"{ "total_count": 0, "check_runs": [] }"#;
        let result = parse_check_runs(json).unwrap();
        assert_eq!(result.total_count, 0);
        assert!(result.check_runs.is_empty());
    }
}
