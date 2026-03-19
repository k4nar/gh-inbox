// The separate review thread states query has been superseded by the unified
// GraphQL query in fetch_pr_graphql.rs, which fetches isResolved alongside all
// other PR data. This module retains only the deserialization test.

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct ReviewThreadStatesResponse {
        data: ReviewThreadStatesData,
    }
    #[derive(Debug, Deserialize)]
    struct ReviewThreadStatesData {
        repository: Option<ReviewThreadRepository>,
    }
    #[derive(Debug, Deserialize)]
    struct ReviewThreadRepository {
        #[serde(rename = "pullRequest")]
        pull_request: Option<ReviewThreadPullRequest>,
    }
    #[derive(Debug, Deserialize)]
    struct ReviewThreadPullRequest {
        #[serde(rename = "reviewThreads")]
        review_threads: ReviewThreadConnection,
    }
    #[derive(Debug, Deserialize)]
    struct ReviewThreadConnection {
        nodes: Vec<ReviewThreadNode>,
    }
    #[derive(Debug, Deserialize)]
    struct ReviewThreadNode {
        #[serde(rename = "isResolved")]
        is_resolved: bool,
        comments: ReviewThreadCommentConnection,
    }
    #[derive(Debug, Deserialize)]
    struct ReviewThreadCommentConnection {
        nodes: Vec<ReviewThreadCommentNode>,
    }
    #[derive(Debug, Deserialize)]
    struct ReviewThreadCommentNode {
        #[serde(rename = "databaseId")]
        database_id: Option<i64>,
    }

    #[test]
    fn parses_review_thread_states() {
        let json = r#"
        {
          "data": {
            "repository": {
              "pullRequest": {
                "reviewThreads": {
                  "nodes": [
                    {
                      "isResolved": true,
                      "comments": {
                        "nodes": [
                          { "databaseId": 2001 },
                          { "databaseId": 2002 }
                        ]
                      }
                    },
                    {
                      "isResolved": false,
                      "comments": {
                        "nodes": [
                          { "databaseId": 3001 }
                        ]
                      }
                    }
                  ]
                }
              }
            }
          }
        }
        "#;

        let parsed: ReviewThreadStatesResponse = serde_json::from_str(json).unwrap();
        let mut result = HashMap::new();
        if let Some(repository) = parsed.data.repository {
            if let Some(pull_request) = repository.pull_request {
                result = pull_request
                    .review_threads
                    .nodes
                    .into_iter()
                    .filter_map(|thread| {
                        let root_id = thread
                            .comments
                            .nodes
                            .into_iter()
                            .find_map(|comment| comment.database_id);
                        root_id.map(|id| (id, thread.is_resolved))
                    })
                    .collect();
            }
        }

        assert_eq!(result.get(&2001), Some(&true));
        assert_eq!(result.get(&3001), Some(&false));
    }
}
