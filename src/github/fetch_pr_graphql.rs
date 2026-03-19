use std::collections::HashMap;

use serde::Deserialize;
use serde_json::json;

use crate::api::AppError;
use crate::models::{
    GithubCheckRun, GithubCheckRunList, GithubCommit, GithubCommitAuthor, GithubCommitDetail,
    GithubHead, GithubIssueComment, GithubLabel, GithubPullRequest, GithubReview,
    GithubReviewComment, GithubUser,
};

use super::GithubClient;

const PULL_REQUEST_QUERY: &str = r#"
query PullRequestFull($owner: String!, $repo: String!, $number: Int!) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $number) {
      number
      title
      body
      state
      isDraft
      mergedAt
      additions
      deletions
      changedFiles
      url
      author { login }
      headRefOid
      labels(first: 100) {
        nodes { name color }
      }
      comments(first: 100) {
        nodes {
          databaseId
          author { login }
          body
          createdAt
          url
        }
      }
      reviewThreads(first: 100) {
        nodes {
          isResolved
          comments(first: 100) {
            nodes {
              databaseId
              author { login }
              body
              createdAt
              path
              position
              replyTo { databaseId }
              pullRequestReview { databaseId }
              url
              diffHunk
            }
          }
        }
      }
      allCommits: commits(first: 250) {
        nodes {
          commit {
            oid
            message
            author { name date }
          }
        }
      }
      headCommit: commits(last: 1) {
        nodes {
          commit {
            statusCheckRollup {
              contexts(first: 100) {
                nodes {
                  __typename
                  ... on CheckRun {
                    databaseId
                    name
                    status
                    conclusion
                  }
                }
              }
            }
          }
        }
      }
      reviews(first: 100) {
        nodes {
          databaseId
          author { login }
          state
          body
          submittedAt
          url
        }
      }
    }
  }
}
"#;

const ALLOWED_REVIEW_STATES: &[&str] = &["APPROVED", "CHANGES_REQUESTED", "DISMISSED"];

// -- GraphQL response types (file-local) --

#[derive(Debug, Deserialize)]
struct GqlResponse {
    data: Option<GqlData>,
}

#[derive(Debug, Deserialize)]
struct GqlData {
    repository: Option<GqlRepository>,
}

#[derive(Debug, Deserialize)]
struct GqlRepository {
    #[serde(rename = "pullRequest")]
    pull_request: Option<GqlPullRequest>,
}

#[derive(Debug, Deserialize)]
struct GqlConnection<T> {
    nodes: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct GqlPullRequest {
    number: i64,
    title: String,
    body: Option<String>,
    state: String,
    #[serde(rename = "isDraft")]
    is_draft: bool,
    #[serde(rename = "mergedAt")]
    merged_at: Option<String>,
    additions: Option<i64>,
    deletions: Option<i64>,
    #[serde(rename = "changedFiles")]
    changed_files: Option<i64>,
    url: String,
    author: Option<GqlAuthor>,
    #[serde(rename = "headRefOid")]
    head_ref_oid: String,
    labels: GqlConnection<GqlLabel>,
    comments: GqlConnection<GqlIssueComment>,
    #[serde(rename = "reviewThreads")]
    review_threads: GqlConnection<GqlReviewThread>,
    #[serde(rename = "allCommits")]
    all_commits: GqlConnection<GqlCommitNode>,
    #[serde(rename = "headCommit")]
    head_commit: GqlConnection<GqlHeadCommitNode>,
    reviews: GqlConnection<GqlReview>,
}

#[derive(Debug, Deserialize)]
struct GqlAuthor {
    login: String,
}

#[derive(Debug, Deserialize)]
struct GqlLabel {
    name: String,
    color: String,
}

#[derive(Debug, Deserialize)]
struct GqlIssueComment {
    #[serde(rename = "databaseId")]
    database_id: Option<i64>,
    author: Option<GqlAuthor>,
    body: String,
    #[serde(rename = "createdAt")]
    created_at: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct GqlReviewThread {
    #[serde(rename = "isResolved")]
    is_resolved: bool,
    comments: GqlConnection<GqlReviewComment>,
}

#[derive(Debug, Deserialize)]
struct GqlReviewComment {
    #[serde(rename = "databaseId")]
    database_id: Option<i64>,
    author: Option<GqlAuthor>,
    body: String,
    #[serde(rename = "createdAt")]
    created_at: String,
    path: Option<String>,
    position: Option<i64>,
    #[serde(rename = "replyTo")]
    reply_to: Option<GqlIdRef>,
    #[serde(rename = "pullRequestReview")]
    pull_request_review: Option<GqlIdRef>,
    url: String,
    #[serde(rename = "diffHunk")]
    diff_hunk: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GqlIdRef {
    #[serde(rename = "databaseId")]
    database_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct GqlCommitNode {
    commit: GqlCommit,
}

#[derive(Debug, Deserialize)]
struct GqlCommit {
    oid: String,
    message: String,
    author: Option<GqlCommitAuthor>,
}

#[derive(Debug, Deserialize)]
struct GqlCommitAuthor {
    name: Option<String>,
    date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GqlHeadCommitNode {
    commit: GqlHeadCommit,
}

#[derive(Debug, Deserialize)]
struct GqlHeadCommit {
    #[serde(rename = "statusCheckRollup")]
    status_check_rollup: Option<GqlStatusCheckRollup>,
}

#[derive(Debug, Deserialize)]
struct GqlStatusCheckRollup {
    contexts: GqlConnection<GqlCheckContext>,
}

#[derive(Debug, Deserialize)]
struct GqlCheckContext {
    #[serde(rename = "__typename")]
    typename: String,
    #[serde(rename = "databaseId")]
    database_id: Option<i64>,
    name: Option<String>,
    status: Option<String>,
    conclusion: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GqlReview {
    #[serde(rename = "databaseId")]
    database_id: Option<i64>,
    author: Option<GqlAuthor>,
    state: String,
    body: String,
    #[serde(rename = "submittedAt")]
    submitted_at: Option<String>,
    url: String,
}

// -- Public result type --

pub struct GraphqlPrData {
    pub pull_request: GithubPullRequest,
    pub issue_comments: Vec<GithubIssueComment>,
    pub review_comments: Vec<GithubReviewComment>,
    pub commits: Vec<GithubCommit>,
    pub check_runs: GithubCheckRunList,
    pub reviews: Vec<GithubReview>,
    pub review_thread_states: HashMap<i64, bool>,
}

fn author_login(author: &Option<GqlAuthor>) -> String {
    author
        .as_ref()
        .map(|a| a.login.clone())
        .unwrap_or_else(|| "ghost".to_string())
}

fn convert_pr_state(gql_state: &str) -> String {
    match gql_state {
        "OPEN" => "open".to_string(),
        // GraphQL returns MERGED as a distinct state; REST uses state=closed + merged_at
        "MERGED" | "CLOSED" => "closed".to_string(),
        other => other.to_lowercase(),
    }
}

fn convert(gql_pr: GqlPullRequest) -> GraphqlPrData {
    let state = convert_pr_state(&gql_pr.state);
    let merged_at = if gql_pr.state == "MERGED" {
        // If GraphQL says MERGED, ensure merged_at is set (use the value from the field)
        gql_pr.merged_at.or_else(|| Some(String::new()))
    } else {
        gql_pr.merged_at
    };

    let pull_request = GithubPullRequest {
        number: gql_pr.number,
        title: gql_pr.title,
        body: gql_pr.body,
        state,
        user: GithubUser {
            login: author_login(&gql_pr.author),
        },
        html_url: gql_pr.url,
        head: GithubHead {
            sha: gql_pr.head_ref_oid,
        },
        draft: gql_pr.is_draft,
        merged_at,
        additions: gql_pr.additions,
        deletions: gql_pr.deletions,
        changed_files: gql_pr.changed_files,
        labels: gql_pr
            .labels
            .nodes
            .into_iter()
            .map(|l| GithubLabel {
                name: l.name,
                color: l.color,
            })
            .collect(),
    };

    let issue_comments = gql_pr
        .comments
        .nodes
        .into_iter()
        .filter_map(|c| {
            let id = c.database_id?;
            Some(GithubIssueComment {
                id,
                user: GithubUser {
                    login: author_login(&c.author),
                },
                body: c.body,
                created_at: c.created_at,
                html_url: c.url,
            })
        })
        .collect();

    let mut review_comments = Vec::new();
    let mut review_thread_states = HashMap::new();

    for thread in gql_pr.review_threads.nodes {
        // Track the root comment id for resolved state
        let root_id = thread.comments.nodes.first().and_then(|c| c.database_id);
        if let Some(root_id) = root_id {
            review_thread_states.insert(root_id, thread.is_resolved);
        }

        for comment in thread.comments.nodes {
            let Some(id) = comment.database_id else {
                continue;
            };
            review_comments.push(GithubReviewComment {
                id,
                user: GithubUser {
                    login: author_login(&comment.author),
                },
                body: comment.body,
                created_at: comment.created_at,
                path: comment.path.unwrap_or_default(),
                position: comment.position,
                in_reply_to_id: comment.reply_to.and_then(|r| r.database_id),
                pull_request_review_id: comment.pull_request_review.and_then(|r| r.database_id),
                html_url: comment.url,
                diff_hunk: comment.diff_hunk,
            });
        }
    }

    let commits = gql_pr
        .all_commits
        .nodes
        .into_iter()
        .map(|node| GithubCommit {
            sha: node.commit.oid,
            commit: GithubCommitDetail {
                message: node.commit.message,
                author: GithubCommitAuthor {
                    name: node
                        .commit
                        .author
                        .as_ref()
                        .and_then(|a| a.name.clone())
                        .unwrap_or_else(|| "ghost".to_string()),
                    date: node.commit.author.and_then(|a| a.date).unwrap_or_default(),
                },
            },
        })
        .collect();

    let mut check_runs_vec = Vec::new();
    if let Some(head_node) = gql_pr.head_commit.nodes.into_iter().next()
        && let Some(rollup) = head_node.commit.status_check_rollup
    {
        for ctx in rollup.contexts.nodes {
            if ctx.typename == "CheckRun"
                && let Some(id) = ctx.database_id
            {
                check_runs_vec.push(GithubCheckRun {
                    id,
                    name: ctx.name.unwrap_or_default(),
                    status: ctx.status.map(|s| s.to_lowercase()).unwrap_or_default(),
                    conclusion: ctx.conclusion.map(|c| c.to_lowercase()),
                });
            }
        }
    }
    let check_runs = GithubCheckRunList {
        total_count: check_runs_vec.len() as i64,
        check_runs: check_runs_vec,
    };

    let reviews = gql_pr
        .reviews
        .nodes
        .into_iter()
        .filter_map(|r| {
            let id = r.database_id?;
            if !ALLOWED_REVIEW_STATES.contains(&r.state.as_str()) {
                return None;
            }
            Some(GithubReview {
                id,
                user: GithubUser {
                    login: author_login(&r.author),
                },
                state: r.state,
                body: r.body,
                submitted_at: r.submitted_at.unwrap_or_default(),
                html_url: r.url,
            })
        })
        .collect();

    GraphqlPrData {
        pull_request,
        issue_comments,
        review_comments,
        commits,
        check_runs,
        reviews,
        review_thread_states,
    }
}

pub async fn fetch_pr_graphql(
    github: &GithubClient,
    owner: &str,
    repo: &str,
    number: i64,
) -> Result<GraphqlPrData, AppError> {
    let response: GqlResponse = github
        .post_json(
            "/graphql",
            &json!({
                "query": PULL_REQUEST_QUERY,
                "variables": {
                    "owner": owner,
                    "repo": repo,
                    "number": number,
                },
            }),
        )
        .await?
        .error_for_status()?
        .json()
        .await?;

    let gql_pr = response
        .data
        .and_then(|d| d.repository)
        .and_then(|r| r.pull_request)
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Pull request {owner}/{repo}#{number} not found via GraphQL"
            ))
        })?;

    Ok(convert(gql_pr))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL_RESPONSE: &str = r#"{
      "data": {
        "repository": {
          "pullRequest": {
            "number": 42,
            "title": "Fix bug",
            "body": "Fixes the parser",
            "state": "OPEN",
            "isDraft": false,
            "mergedAt": null,
            "additions": 10,
            "deletions": 2,
            "changedFiles": 1,
            "url": "https://github.com/owner/repo/pull/42",
            "author": { "login": "alice" },
            "headRefOid": "deadbeef",
            "labels": {
              "nodes": [
                { "name": "bug", "color": "d73a4a" }
              ]
            },
            "comments": {
              "nodes": [
                {
                  "databaseId": 100,
                  "author": { "login": "bob" },
                  "body": "Looks good!",
                  "createdAt": "2025-01-01T00:00:00Z",
                  "url": "https://github.com/owner/repo/pull/42#issuecomment-100"
                }
              ]
            },
            "reviewThreads": {
              "nodes": [
                {
                  "isResolved": true,
                  "comments": {
                    "nodes": [
                      {
                        "databaseId": 200,
                        "author": { "login": "carol" },
                        "body": "Nit: rename this",
                        "createdAt": "2025-01-01T00:00:00Z",
                        "path": "src/main.rs",
                        "position": 10,
                        "replyTo": null,
                        "pullRequestReview": { "databaseId": 50 },
                        "url": "https://github.com/owner/repo/pull/42#discussion_r200",
                        "diffHunk": "@@ -1,4 +1,5 @@"
                      },
                      {
                        "databaseId": 201,
                        "author": { "login": "alice" },
                        "body": "Done!",
                        "createdAt": "2025-01-02T00:00:00Z",
                        "path": "src/main.rs",
                        "position": 10,
                        "replyTo": { "databaseId": 200 },
                        "pullRequestReview": { "databaseId": 51 },
                        "url": "https://github.com/owner/repo/pull/42#discussion_r201",
                        "diffHunk": null
                      }
                    ]
                  }
                }
              ]
            },
            "allCommits": {
              "nodes": [
                {
                  "commit": {
                    "oid": "abc123",
                    "message": "Fix parser bug",
                    "author": { "name": "Alice", "date": "2025-01-01T00:00:00Z" }
                  }
                }
              ]
            },
            "headCommit": {
              "nodes": [
                {
                  "commit": {
                    "statusCheckRollup": {
                      "contexts": {
                        "nodes": [
                          {
                            "__typename": "CheckRun",
                            "databaseId": 1,
                            "name": "CI",
                            "status": "COMPLETED",
                            "conclusion": "SUCCESS"
                          },
                          {
                            "__typename": "StatusContext",
                            "databaseId": null,
                            "name": null,
                            "status": null,
                            "conclusion": null
                          }
                        ]
                      }
                    }
                  }
                }
              ]
            },
            "reviews": {
              "nodes": [
                {
                  "databaseId": 1,
                  "author": { "login": "bob" },
                  "state": "APPROVED",
                  "body": "LGTM",
                  "submittedAt": "2025-06-01T10:00:00Z",
                  "url": "https://github.com/owner/repo/pull/42#pullrequestreview-1"
                },
                {
                  "databaseId": 2,
                  "author": { "login": "eve" },
                  "state": "COMMENTED",
                  "body": "hmm",
                  "submittedAt": "2025-06-01T11:00:00Z",
                  "url": "https://github.com/owner/repo/pull/42#pullrequestreview-2"
                }
              ]
            }
          }
        }
      }
    }"#;

    #[test]
    fn parses_and_converts_full_response() {
        let response: GqlResponse = serde_json::from_str(FULL_RESPONSE).unwrap();
        let gql_pr = response
            .data
            .unwrap()
            .repository
            .unwrap()
            .pull_request
            .unwrap();
        let data = convert(gql_pr);

        // PR metadata
        assert_eq!(data.pull_request.number, 42);
        assert_eq!(data.pull_request.title, "Fix bug");
        assert_eq!(data.pull_request.state, "open");
        assert_eq!(data.pull_request.user.login, "alice");
        assert_eq!(data.pull_request.head.sha, "deadbeef");
        assert!(!data.pull_request.draft);
        assert!(data.pull_request.merged_at.is_none());
        assert_eq!(data.pull_request.additions, Some(10));
        assert_eq!(data.pull_request.labels.len(), 1);
        assert_eq!(data.pull_request.labels[0].name, "bug");

        // Issue comments
        assert_eq!(data.issue_comments.len(), 1);
        assert_eq!(data.issue_comments[0].id, 100);
        assert_eq!(data.issue_comments[0].user.login, "bob");
        assert_eq!(data.issue_comments[0].body, "Looks good!");

        // Review comments (flattened from threads)
        assert_eq!(data.review_comments.len(), 2);
        assert_eq!(data.review_comments[0].id, 200);
        assert_eq!(data.review_comments[0].path, "src/main.rs");
        assert!(data.review_comments[0].in_reply_to_id.is_none());
        assert_eq!(data.review_comments[0].pull_request_review_id, Some(50));
        assert_eq!(data.review_comments[1].id, 201);
        assert_eq!(data.review_comments[1].in_reply_to_id, Some(200));
        assert_eq!(
            data.review_comments[0].diff_hunk,
            Some("@@ -1,4 +1,5 @@".to_string())
        );

        // Review thread states
        assert_eq!(data.review_thread_states.get(&200), Some(&true));

        // Commits
        assert_eq!(data.commits.len(), 1);
        assert_eq!(data.commits[0].sha, "abc123");
        assert_eq!(data.commits[0].commit.message, "Fix parser bug");
        assert_eq!(data.commits[0].commit.author.name, "Alice");

        // Check runs (only CheckRun, not StatusContext)
        assert_eq!(data.check_runs.total_count, 1);
        assert_eq!(data.check_runs.check_runs[0].name, "CI");
        assert_eq!(data.check_runs.check_runs[0].status, "completed");
        assert_eq!(
            data.check_runs.check_runs[0].conclusion,
            Some("success".to_string())
        );

        // Reviews (COMMENTED filtered out)
        assert_eq!(data.reviews.len(), 1);
        assert_eq!(data.reviews[0].id, 1);
        assert_eq!(data.reviews[0].state, "APPROVED");
    }

    #[test]
    fn merged_state_mapping() {
        let json = r#"{
          "data": {
            "repository": {
              "pullRequest": {
                "number": 1, "title": "T", "body": null, "state": "MERGED",
                "isDraft": false, "mergedAt": "2025-06-01T00:00:00Z",
                "additions": null, "deletions": null, "changedFiles": null,
                "url": "u", "author": { "login": "a" }, "headRefOid": "s",
                "labels": { "nodes": [] }, "comments": { "nodes": [] },
                "reviewThreads": { "nodes": [] },
                "allCommits": { "nodes": [] },
                "headCommit": { "nodes": [{ "commit": { "statusCheckRollup": null } }] },
                "reviews": { "nodes": [] }
              }
            }
          }
        }"#;
        let response: GqlResponse = serde_json::from_str(json).unwrap();
        let gql_pr = response
            .data
            .unwrap()
            .repository
            .unwrap()
            .pull_request
            .unwrap();
        let data = convert(gql_pr);

        assert_eq!(data.pull_request.state, "closed");
        assert_eq!(
            data.pull_request.merged_at,
            Some("2025-06-01T00:00:00Z".to_string())
        );
    }

    #[test]
    fn null_author_becomes_ghost() {
        let json = r#"{
          "data": {
            "repository": {
              "pullRequest": {
                "number": 1, "title": "T", "body": null, "state": "OPEN",
                "isDraft": false, "mergedAt": null,
                "additions": null, "deletions": null, "changedFiles": null,
                "url": "u", "author": null, "headRefOid": "s",
                "labels": { "nodes": [] },
                "comments": {
                  "nodes": [
                    { "databaseId": 1, "author": null, "body": "hi", "createdAt": "2025-01-01T00:00:00Z", "url": "u" }
                  ]
                },
                "reviewThreads": { "nodes": [] },
                "allCommits": { "nodes": [] },
                "headCommit": { "nodes": [{ "commit": { "statusCheckRollup": null } }] },
                "reviews": { "nodes": [] }
              }
            }
          }
        }"#;
        let response: GqlResponse = serde_json::from_str(json).unwrap();
        let gql_pr = response
            .data
            .unwrap()
            .repository
            .unwrap()
            .pull_request
            .unwrap();
        let data = convert(gql_pr);

        assert_eq!(data.pull_request.user.login, "ghost");
        assert_eq!(data.issue_comments[0].user.login, "ghost");
    }

    #[test]
    fn no_check_runs_when_rollup_is_null() {
        let json = r#"{
          "data": {
            "repository": {
              "pullRequest": {
                "number": 1, "title": "T", "body": null, "state": "OPEN",
                "isDraft": false, "mergedAt": null,
                "additions": null, "deletions": null, "changedFiles": null,
                "url": "u", "author": { "login": "a" }, "headRefOid": "s",
                "labels": { "nodes": [] }, "comments": { "nodes": [] },
                "reviewThreads": { "nodes": [] },
                "allCommits": { "nodes": [] },
                "headCommit": { "nodes": [{ "commit": { "statusCheckRollup": null } }] },
                "reviews": { "nodes": [] }
              }
            }
          }
        }"#;
        let response: GqlResponse = serde_json::from_str(json).unwrap();
        let gql_pr = response
            .data
            .unwrap()
            .repository
            .unwrap()
            .pull_request
            .unwrap();
        let data = convert(gql_pr);

        assert_eq!(data.check_runs.total_count, 0);
        assert!(data.check_runs.check_runs.is_empty());
    }
}
