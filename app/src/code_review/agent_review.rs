//! Review data handed to third-party CLI agents as a prompt.

use std::collections::HashMap;
use std::ops::Range;

use serde::{Deserialize, Serialize};
use warp_editor::render::model::LineCount;
use warp_multi_agent_api::diff_hunk as diff_hunk_api;

use crate::code_review::comments::{AttachedReviewComment, ReviewCommentBatch};

/// The checked-out head of a repository.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurrentHead {
    BranchName(String),
    HeadlessCommitSha(String),
}

impl CurrentHead {
    pub fn title(&self) -> String {
        match self {
            CurrentHead::BranchName(name) => name.clone(),
            CurrentHead::HeadlessCommitSha(sha) => {
                let short = sha.chars().take(7).collect::<String>();
                format!("Commit {short}")
            }
        }
    }
}

/// The base a diff is computed against.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffBase {
    BranchName(String),
    HeadlessCommitSha(String),
    UncommittedChanges,
}

/// A simplified diff hunk.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffSetHunk {
    pub line_range: Range<LineCount>,
    pub diff_content: String,
    pub lines_added: u32,
    pub lines_removed: u32,
}

/// Review comments sent to a CLI agent, with the diff hunks they are attached to.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentReviewCommentBatch {
    pub comments: Vec<AttachedReviewComment>,
    /// All diff hunks that have comments in this batch attached to them, grouped by file name.
    pub diff_set: HashMap<String, Vec<DiffSetHunk>>,
}

impl AgentReviewCommentBatch {
    pub fn review_comments(&self) -> ReviewCommentBatch {
        ReviewCommentBatch::from_comments(self.comments.clone())
    }
}

impl From<CurrentHead> for warp_multi_agent_api::CurrentRef {
    fn from(value: CurrentHead) -> Self {
        Self {
            r#ref: Some(match value {
                CurrentHead::BranchName(name) => {
                    warp_multi_agent_api::current_ref::Ref::BranchName(name)
                }
                CurrentHead::HeadlessCommitSha(sha) => {
                    warp_multi_agent_api::current_ref::Ref::HeadlessCommitSha(sha)
                }
            }),
        }
    }
}

impl From<CurrentHead> for diff_hunk_api::Current {
    fn from(value: CurrentHead) -> Self {
        match value {
            CurrentHead::BranchName(name) => diff_hunk_api::Current::CurrentBranchName(name),
            CurrentHead::HeadlessCommitSha(sha) => {
                diff_hunk_api::Current::CurrentHeadlessCommitSha(sha)
            }
        }
    }
}

impl From<DiffBase> for warp_multi_agent_api::BaseRef {
    fn from(value: DiffBase) -> Self {
        Self {
            r#ref: Some(match value {
                DiffBase::BranchName(name) => warp_multi_agent_api::base_ref::Ref::BranchName(name),
                DiffBase::HeadlessCommitSha(sha) => {
                    warp_multi_agent_api::base_ref::Ref::HeadlessCommitSha(sha)
                }
                DiffBase::UncommittedChanges => {
                    warp_multi_agent_api::base_ref::Ref::UncommittedChanges(())
                }
            }),
        }
    }
}

impl From<DiffBase> for diff_hunk_api::Base {
    fn from(value: DiffBase) -> Self {
        match value {
            DiffBase::BranchName(branch_name) => diff_hunk_api::Base::BaseBranchName(branch_name),
            DiffBase::HeadlessCommitSha(sha) => diff_hunk_api::Base::BaseHeadlessCommitSha(sha),
            DiffBase::UncommittedChanges =>
            {
                #[warn(clippy::unit_arg)]
                diff_hunk_api::Base::UncommittedChanges(())
            }
        }
    }
}
