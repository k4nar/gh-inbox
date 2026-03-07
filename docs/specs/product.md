# gh-inbox

## The story

I review a lot of Github PR every day. At any given time there can be a dozen of PR in which I’m involved, I need to keep track of what’s happening on each of them.

My current flow is to use the Github notifications by email. I can use inbox zero to keep track of the PR to follow at the moment, and archive the thread once the PR is merged. The great thing about email notification is that I get a new message for each new comment and each new commit.

However, mails don’t always offer the best UX. It’s hard to follow every conversation, there are a lot of unnecessary information in each message and it makes handling my other emails harder than it should be.

I’ve tried several other tools (official Github Notification page, `gh-dash`, Slack integration…) but none of them offer the same level of control and the same level of information. For example a lot of notification don’t highlight what has changed when there is a notification, so you need to open the PR to figure it out. This is not efficient when getting hundreds of notifications every day.

## What is gh-inbox

`gh-inbox` is a web dashboard for Github Notifications, combining the effectiveness of a specialized UI with the power of email notifications.

It offers an Inbox to read all your notifications by PR, track every thread of conversation for each PR, and allow know to exactly what has changed without needing to open the PR.

## Design

The UI is meant to be effective. It follows the design system of Github, with some leeway. It can be themed easily.

## Job Stories

Template:
```
*When* <situation/context>,
*I want to* <motivations>,
*so I can* <expected outcome>.
```

1. New PR

*When* a dev opens a PR for which I’m notified,
*I want to* view it in my inbox with the name of the author, its title and the repository,
*so that I can* review it.

*When* a dev opens a PR for which I’m notified,
*I want to* know why I am notified (assigned to the review, ping, codewoner, etc…),
*so that I can* review it.

2. Keeping track of PR

*When* I am in my Inbox
*I want to* know the status of the CI for each PR,
*so that I can* avoid reviewing PRs with failing tests or know that there are some tests I should fix.

*When* I am in my Inbox
*I want to* be able to filter the PR by repo or org
*so that I can* focus on the PRs important for my current project.
 
*When* I am in my Inbox
*I want to* be able to filter the PR for which I’m notified due to being part of a specific codewoner team,
*so that I can* focus on the PRs to review in the context of a specific team.

3. Overview of each PR

*When* I click on a PR,
*I want to* see the new comments since I’ve opened it,
*so I can* read only the new information.

*When* I click on a PR,
*I want to* see the new commits pushed since I’ve opened it,
*so I can* know if there is something new to review.

*When* I click on a PR,
*I want to* see the comments grouped by thread,
*so I can* keep track of the different conversations.

4. Inbox 0

*When* I open a notification,
*I want to* mark it as read but keep the PR in list of PR to follow,
*so I can* go back to this PR later.

*When* I archive a PR,
*I want to* not see it anymore in the list of PRs to follow,
*so I can* focus on other PRs.

*When* I archive a PR,
*I want to* be able to see it in another inbox,
*so I can* go back to it later and eventually unarchive it.
