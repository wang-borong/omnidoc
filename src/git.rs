use crate::constants::git_commits;
use crate::constants::git_refs;
use git2::{Repository, Signature};
use serde::Serialize;
use std::io::{self, Write};
use std::path::Path;
use std::str;

pub fn git_clone<P>(url: &str, p: P, recurse: bool) -> Result<(), git2::Error>
where
    P: AsRef<Path>,
{
    let repository = if recurse {
        Repository::clone_recurse(url, p)?
    } else {
        Repository::clone(url, p)?
    };
    // omnidoc-libs verifies byte-for-byte payload checksums. A user's global
    // core.autocrlf setting must not rewrite text resources during checkout.
    checkout_without_line_ending_conversion(&repository)?;

    Ok(())
}

fn repository_signature(repo: &Repository) -> Result<Signature<'static>, git2::Error> {
    repo.signature()
        .or_else(|_| Signature::now("OmniDoc", "omnidoc@example.invalid"))
}

fn checkout_without_line_ending_conversion(repository: &Repository) -> Result<(), git2::Error> {
    repository.config()?.set_bool("core.autocrlf", false)?;
    repository.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
}

/// Unlike regular "git init", this example shows how to create an initial empty
/// commit in the repository. This is the helper function that does that.
fn create_initial_commit(repo: &Repository) -> Result<(), git2::Error> {
    // First use the config to initialize a commit signature for the user.
    let sig = repository_signature(repo)?;

    // Now let's create an empty tree for this commit
    let tree_id = {
        let mut index = repo.index()?;

        // Outside of this example, you could call index.add_path()
        // here to put actual files into the index. For our purposes, we'll
        // leave it empty for now.

        index.write_tree()?
    };

    let tree = repo.find_tree(tree_id)?;

    // Ready to create the initial commit.
    //
    // Normally creating a commit would involve looking up the current HEAD
    // commit and making that be the parent of the initial commit, but here this
    // is the first commit so there will be no parent.
    repo.commit(
        Some(git_refs::HEAD),
        &sig,
        &sig,
        git_commits::INITIAL_COMMIT_MSG,
        &tree,
        &[],
    )?;

    Ok(())
}

pub fn git_init<P>(p: P, commit: bool) -> Result<(), git2::Error>
where
    P: AsRef<Path>,
{
    let repo = Repository::init(p)?;

    if commit {
        create_initial_commit(&repo)?;
    }

    Ok(())
}

pub fn git_add<P>(repo: P, files: &[&str], update: bool) -> Result<(), git2::Error>
where
    P: AsRef<Path>,
{
    let repo = Repository::open(&repo)?;
    let mut index = repo.index()?;

    if update {
        index.update_all(files.iter(), None)?;
    } else {
        index.add_all(files.iter(), git2::IndexAddOption::DEFAULT, None)?;
    }

    index.write()?;

    Ok(())
}

/// Stage new, modified, renamed, and deleted files while respecting ignore rules.
pub fn git_stage_all<P>(repo: P) -> Result<(), git2::Error>
where
    P: AsRef<Path>,
{
    let repo = Repository::open(repo)?;
    let mut index = repo.index()?;
    index.add_all(["*"], git2::IndexAddOption::DEFAULT, None)?;
    index.update_all(["*"], None)?;
    index.write()
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GitWorktreeChange {
    pub path: String,
    pub index: Option<String>,
    pub worktree: Option<String>,
    pub conflicted: bool,
}

pub fn git_worktree_changes<P>(repo: P) -> Result<Vec<GitWorktreeChange>, git2::Error>
where
    P: AsRef<Path>,
{
    let repo = Repository::open(repo)?;
    let mut options = git2::StatusOptions::new();
    options.include_untracked(true).recurse_untracked_dirs(true);

    let mut changes = repo
        .statuses(Some(&mut options))?
        .iter()
        .map(|entry| {
            let status = entry.status();
            GitWorktreeChange {
                path: String::from_utf8_lossy(entry.path_bytes()).to_string(),
                index: index_change(status).map(str::to_string),
                worktree: worktree_change(status).map(str::to_string),
                conflicted: status.contains(git2::Status::CONFLICTED),
            }
        })
        .collect::<Vec<_>>();
    changes.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(changes)
}

pub fn git_has_commits<P>(repo: P) -> Result<bool, git2::Error>
where
    P: AsRef<Path>,
{
    let repo = Repository::open(repo)?;
    let result = match repo.head() {
        Ok(head) => {
            head.peel_to_commit()?;
            Ok(true)
        }
        Err(error) if error.code() == git2::ErrorCode::UnbornBranch => Ok(false),
        Err(error) => Err(error),
    };
    result
}

fn index_change(status: git2::Status) -> Option<&'static str> {
    if status.contains(git2::Status::INDEX_NEW) {
        Some("added")
    } else if status.contains(git2::Status::INDEX_MODIFIED) {
        Some("modified")
    } else if status.contains(git2::Status::INDEX_DELETED) {
        Some("deleted")
    } else if status.contains(git2::Status::INDEX_RENAMED) {
        Some("renamed")
    } else if status.contains(git2::Status::INDEX_TYPECHANGE) {
        Some("type_changed")
    } else {
        None
    }
}

fn worktree_change(status: git2::Status) -> Option<&'static str> {
    if status.contains(git2::Status::WT_NEW) {
        Some("untracked")
    } else if status.contains(git2::Status::WT_MODIFIED) {
        Some("modified")
    } else if status.contains(git2::Status::WT_DELETED) {
        Some("deleted")
    } else if status.contains(git2::Status::WT_RENAMED) {
        Some("renamed")
    } else if status.contains(git2::Status::WT_TYPECHANGE) {
        Some("type_changed")
    } else {
        None
    }
}

pub fn git_commit<P>(repo: P, msg: &str) -> Result<(), git2::Error>
where
    P: AsRef<Path>,
{
    let repo = Repository::open(&repo)?;

    let mut index = repo.index()?;
    let oid = index.write_tree()?;
    let signature = repository_signature(&repo)?;
    let parent_commit = match repo.head() {
        Ok(head) => Some(head.peel_to_commit()?),
        Err(error) if error.code() == git2::ErrorCode::UnbornBranch => None,
        Err(error) => return Err(error),
    };
    let parents = parent_commit.iter().collect::<Vec<_>>();
    let tree = repo.find_tree(oid)?;
    repo.commit(
        Some(git_refs::HEAD),
        &signature,
        &signature,
        msg,
        &tree,
        &parents,
    )?;

    Ok(())
}

fn do_fetch<'a>(
    repo: &'a git2::Repository,
    refs: &[&str],
    remote: &'a mut git2::Remote,
) -> Result<git2::AnnotatedCommit<'a>, git2::Error> {
    let mut cb = git2::RemoteCallbacks::new();

    // Print out our transfer progress.
    cb.transfer_progress(|stats| {
        if stats.received_objects() == stats.total_objects() {
            //print!(
            //    "Resolving deltas {}/{}\r",
            //    stats.indexed_deltas(),
            //    stats.total_deltas()
            //);
        } else if stats.total_objects() > 0 {
            //print!(
            //    "Received {}/{} objects ({}) in {} bytes\r",
            //    stats.received_objects(),
            //    stats.total_objects(),
            //    stats.indexed_objects(),
            //    stats.received_bytes()
            //);
        }
        let _ = io::stdout().flush();
        true
    });

    let mut fo = git2::FetchOptions::new();
    fo.remote_callbacks(cb);
    // Always fetch all tags.
    // Perform a download and also update tips
    fo.download_tags(git2::AutotagOption::All);
    //println!("Fetching {} for repo", remote.name().unwrap());
    remote.fetch(refs, Some(&mut fo), None)?;

    // If there are local objects (we got a thin pack), then tell the user
    // how many objects we saved from having to cross the network.
    let stats = remote.stats();
    if stats.local_objects() > 0 {
        //println!(
        //    "\rReceived {}/{} objects in {} bytes (used {} local \
        //     objects)",
        //    stats.indexed_objects(),
        //    stats.total_objects(),
        //    stats.received_bytes(),
        //    stats.local_objects()
        //);
    } else {
        //println!(
        //    "\rReceived {}/{} objects in {} bytes",
        //    stats.indexed_objects(),
        //    stats.total_objects(),
        //    stats.received_bytes()
        //);
    }

    let fetch_head = repo.find_reference(git_refs::FETCH_HEAD)?;
    repo.reference_to_annotated_commit(&fetch_head)
}

fn fast_forward(
    repo: &Repository,
    lb: &mut git2::Reference,
    rc: &git2::AnnotatedCommit,
) -> Result<(), git2::Error> {
    let name = lb
        .name()
        .map(str::to_string)
        .unwrap_or_else(|_| String::from_utf8_lossy(lb.name_bytes()).to_string());
    let msg = format!("Fast-Forward: Setting {} to id: {}", name, rc.id());
    //println!("{}", msg);
    lb.set_target(rc.id(), &msg)?;
    repo.set_head(&name)?;
    repo.checkout_head(Some(
        git2::build::CheckoutBuilder::default()
            // For some reason the force is required to make the working directory actually get updated
            // I suspect we should be adding some logic to handle dirty working directory states
            // but this is just an example so maybe not.
            .force(),
    ))?;
    Ok(())
}

fn normal_merge(
    repo: &Repository,
    local: &git2::AnnotatedCommit,
    remote: &git2::AnnotatedCommit,
) -> Result<(), git2::Error> {
    let local_tree = repo.find_commit(local.id())?.tree()?;
    let remote_tree = repo.find_commit(remote.id())?.tree()?;
    let ancestor = repo
        .find_commit(repo.merge_base(local.id(), remote.id())?)?
        .tree()?;
    let mut idx = repo.merge_trees(&ancestor, &local_tree, &remote_tree, None)?;

    if idx.has_conflicts() {
        //println!("Merge conflicts detected...");
        repo.checkout_index(Some(&mut idx), None)?;
        return Ok(());
    }
    let result_tree = repo.find_tree(idx.write_tree_to(repo)?)?;
    // now create the merge commit
    let msg = format!("Merge: {} into {}", remote.id(), local.id());
    let sig = repo.signature()?;
    let local_commit = repo.find_commit(local.id())?;
    let remote_commit = repo.find_commit(remote.id())?;
    // Do our merge commit and set current branch head to that commit.
    let _merge_commit = repo.commit(
        Some(git_refs::HEAD),
        &sig,
        &sig,
        &msg,
        &result_tree,
        &[&local_commit, &remote_commit],
    )?;
    // Set working tree to match head.
    repo.checkout_head(None)?;
    Ok(())
}

fn do_merge<'a>(
    repo: &'a Repository,
    remote_branch: &str,
    fetch_commit: git2::AnnotatedCommit<'a>,
) -> Result<(), git2::Error> {
    // 1. do a merge analysis
    let analysis = repo.merge_analysis(&[&fetch_commit])?;

    // 2. Do the appropriate merge
    if analysis.0.is_fast_forward() {
        //println!("Doing a fast forward");
        // do a fast forward
        let refname = format!("{}{}", git_refs::REFS_HEADS_PREFIX, remote_branch);
        match repo.find_reference(&refname) {
            Ok(mut r) => {
                fast_forward(repo, &mut r, &fetch_commit)?;
            }
            Err(_) => {
                // The branch doesn't exist so just set the reference to the
                // commit directly. Usually this is because you are pulling
                // into an empty repository.
                repo.reference(
                    &refname,
                    fetch_commit.id(),
                    true,
                    &format!("Setting {} to {}", remote_branch, fetch_commit.id()),
                )?;
                repo.set_head(&refname)?;
                repo.checkout_head(Some(
                    git2::build::CheckoutBuilder::default()
                        .allow_conflicts(true)
                        .conflict_style_merge(true)
                        .force(),
                ))?;
            }
        };
    } else if analysis.0.is_normal() {
        // do a normal merge
        let head_commit = repo.reference_to_annotated_commit(&repo.head()?)?;
        normal_merge(repo, &head_commit, &fetch_commit)?;
    } else {
        //println!("Nothing to do...");
    }

    Ok(())
}

pub fn git_pull<P>(repo: P, remote: &str, branch: &str) -> Result<(), git2::Error>
where
    P: AsRef<Path>,
{
    let repo = Repository::open(repo)?;
    let mut remote = repo.find_remote(remote)?;
    let fetch_commit = do_fetch(&repo, &[branch], &mut remote)?;
    do_merge(&repo, branch, fetch_commit)
}

/// Resolve and check out a tag, branch, or commit as a detached HEAD.
pub fn git_checkout_revision<P>(repo: P, revision: &str) -> Result<git2::Oid, git2::Error>
where
    P: AsRef<Path>,
{
    let repo = Repository::open(repo)?;
    let mut status_options = git2::StatusOptions::new();
    status_options
        .include_untracked(true)
        .recurse_untracked_dirs(true);
    if !repo.statuses(Some(&mut status_options))?.is_empty() {
        return Err(git2::Error::from_str(
            "refusing to check out a revision in a dirty repository",
        ));
    }
    let object = resolve_revision(&repo, revision)?;
    let commit = object.peel_to_commit()?;
    repo.checkout_tree(
        commit.as_object(),
        Some(git2::build::CheckoutBuilder::new().safe()),
    )?;
    repo.set_head_detached(commit.id())?;
    Ok(commit.id())
}

fn resolve_revision<'repo>(
    repo: &'repo Repository,
    revision: &str,
) -> Result<git2::Object<'repo>, git2::Error> {
    for candidate in [
        revision.to_string(),
        format!("refs/tags/{revision}"),
        format!("refs/remotes/origin/{revision}"),
    ] {
        if let Ok(object) = repo.revparse_single(&candidate) {
            return Ok(object);
        }
    }
    repo.revparse_single(revision)
}

pub fn is_git_repo<P>(repo: P) -> bool
where
    P: AsRef<Path>,
{
    Repository::open(repo)
        .map(|repository| !repository.is_bare())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Signature;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "omnidoc_{}_{}_{}",
            name,
            std::process::id(),
            unique
        ))
    }

    fn create_source_repo(path: &Path) {
        fs::create_dir_all(path).expect("create source dir");
        let repo = Repository::init(path).expect("init source repo");
        fs::write(path.join("README.md"), b"# source\n").expect("write source file");

        let mut index = repo.index().expect("open index");
        index
            .add_path(Path::new("README.md"))
            .expect("add source file");
        index.write().expect("persist source index");
        let tree_id = index.write_tree().expect("write tree");
        let tree = repo.find_tree(tree_id).expect("find tree");
        let sig = Signature::now("OmniDoc Test", "omnidoc@example.invalid").expect("signature");
        repo.commit(
            Some(git_refs::HEAD),
            &sig,
            &sig,
            "Initial test commit",
            &tree,
            &[],
        )
        .expect("commit source repo");
    }

    #[test]
    fn test_git_clone() {
        let root = temp_dir_path("git_clone");
        let source = root.join("source");
        let target = root.join("target");
        create_source_repo(&source);

        git_clone(source.to_str().expect("source path"), &target, false).expect("clone local repo");

        assert!(target.join(".git").exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn checkout_restores_repository_bytes_without_line_ending_conversion() {
        let root = temp_dir_path("git_checkout_bytes");
        let source = root.join("source");
        let target = root.join("target");
        create_source_repo(&source);
        git_clone(source.to_str().expect("source path"), &target, false).expect("clone repo");
        fs::write(target.join("README.md"), b"source\r\n").expect("converted checkout");
        let repository = Repository::open(&target).expect("target repository");

        super::checkout_without_line_ending_conversion(&repository).expect("clean checkout");

        assert_eq!(
            fs::read(target.join("README.md")).expect("checked out bytes"),
            b"# source\n"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn test_git_init() {
        let target = temp_dir_path("git_init");

        git_init(&target, false).expect("init repo");

        assert!(target.join(".git").exists());

        let _ = fs::remove_dir_all(target);
    }

    #[test]
    fn git_commit_creates_the_first_commit_in_an_unborn_repository() {
        let target = temp_dir_path("git_first_commit");
        git_init(&target, false).expect("init repo without commit");
        fs::write(target.join("README.md"), b"# first\n").expect("write first file");
        git_add(&target, &["*"], false).expect("stage first file");

        git_commit(&target, "First real commit").expect("create first commit");

        let repository = Repository::open(&target).expect("open repository");
        let commit = repository
            .head()
            .expect("repository head")
            .peel_to_commit()
            .expect("head commit");
        assert_eq!(
            commit.message().expect("commit message"),
            "First real commit"
        );
        assert_eq!(commit.parent_count(), 0);

        let _ = fs::remove_dir_all(target);
    }

    #[test]
    fn worktree_changes_report_index_and_worktree_state() {
        let target = temp_dir_path("git_status");
        create_source_repo(&target);
        fs::write(target.join("README.md"), b"modified contents\n").expect("modify tracked file");
        fs::write(target.join("notes.md"), b"untracked\n").expect("write untracked file");

        let changes = git_worktree_changes(&target).expect("worktree changes");
        assert!(
            changes.iter().any(|change| {
                change.path == "README.md"
                    && change.index.is_none()
                    && change.worktree.as_deref() == Some("modified")
            }),
            "changes: {changes:?}"
        );
        assert!(changes.iter().any(|change| {
            change.path == "notes.md"
                && change.index.is_none()
                && change.worktree.as_deref() == Some("untracked")
        }));

        let _ = fs::remove_dir_all(target);
    }

    #[test]
    fn stage_all_records_tracked_deletions() {
        let target = temp_dir_path("git_stage_deletion");
        create_source_repo(&target);
        fs::remove_file(target.join("README.md")).expect("remove tracked file");

        git_stage_all(&target).expect("stage deletion");
        git_commit(&target, "Remove README").expect("commit deletion");

        let repository = Repository::open(&target).expect("open repository");
        let tree = repository
            .head()
            .expect("repository head")
            .peel_to_commit()
            .expect("deletion commit")
            .tree()
            .expect("commit tree");
        assert!(tree.get_path(Path::new("README.md")).is_err());

        let _ = fs::remove_dir_all(target);
    }

    #[test]
    fn checks_out_named_revision_without_overwriting_dirty_files() {
        let root = temp_dir_path("git_checkout_revision");
        let source = root.join("source");
        let target = root.join("target");
        create_source_repo(&source);
        let source_repo = Repository::open(&source).expect("source repository");
        let first = source_repo
            .head()
            .expect("source head")
            .target()
            .expect("source commit");
        let first_object = source_repo.find_object(first, None).expect("source object");
        source_repo
            .tag_lightweight("v1.0.0", &first_object, false)
            .expect("create tag");

        git_clone(source.to_str().expect("source path"), &target, false).expect("clone repo");
        let checked_out = git_checkout_revision(&target, "v1.0.0").expect("checkout tag");
        assert_eq!(checked_out, first);
        let target_repo = Repository::open(&target).expect("target repository");
        assert!(target_repo.head_detached().expect("detached status"));

        fs::write(target.join("README.md"), b"dirty\n").expect("dirty file");
        assert!(git_checkout_revision(&target, "v1.0.0").is_err());

        let _ = fs::remove_dir_all(root);
    }
}
