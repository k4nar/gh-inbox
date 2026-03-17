use crate::models::GithubCommit;

use super::GithubClient;

pub async fn fetch_commits(
    github: &GithubClient,
    owner: &str,
    repo: &str,
    number: i64,
) -> Result<Vec<GithubCommit>, reqwest::Error> {
    let path = format!("/repos/{owner}/{repo}/pulls/{number}/commits");
    github.get(&path).await?.error_for_status()?.json().await
}

#[cfg(test)]
mod tests {
    use crate::models::GithubCommit;

    fn parse_commits(json: &str) -> Result<Vec<GithubCommit>, serde_json::Error> {
        serde_json::from_str(json)
    }

    #[test]
    fn parse_valid_commits() {
        let json = r#"[{
            "sha": "abc123",
            "commit": {
                "message": "Fix parser bug",
                "author": { "name": "Alice", "date": "2025-01-01T00:00:00Z" }
            }
        }]"#;
        let commits = parse_commits(json).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].sha, "abc123");
        assert_eq!(commits[0].commit.message, "Fix parser bug");
        assert_eq!(commits[0].commit.author.name, "Alice");
        assert_eq!(commits[0].commit.author.date, "2025-01-01T00:00:00Z");
    }

    #[test]
    fn parse_empty_commits() {
        let commits = parse_commits("[]").unwrap();
        assert!(commits.is_empty());
    }

    #[test]
    fn parse_multiple_commits() {
        let json = r#"[
            {
                "sha": "aaa",
                "commit": { "message": "First", "author": { "name": "A", "date": "2025-01-01T00:00:00Z" } }
            },
            {
                "sha": "bbb",
                "commit": { "message": "Second", "author": { "name": "B", "date": "2025-01-02T00:00:00Z" } }
            }
        ]"#;
        let commits = parse_commits(json).unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].sha, "aaa");
        assert_eq!(commits[1].sha, "bbb");
    }
}
