use crate::constants::git_commits;
use crate::constants::git_refs;
use git2::Repository;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
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

fn checkout_without_line_ending_conversion(repository: &Repository) -> Result<(), git2::Error> {
    repository.config()?.set_bool("core.autocrlf", false)?;
    repository.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
}

/// Unlike regular "git init", this example shows how to create an initial empty
/// commit in the repository. This is the helper function that does that.
fn create_initial_commit(repo: &Repository) -> Result<(), git2::Error> {
    // First use the config to initialize a commit signature for the user.
    let sig = repo.signature()?;

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

    let cb = &mut |path: &Path, _matched_spec: &[u8]| -> i32 {
        let status = match repo.status_file(path) {
            Ok(s) => s,
            Err(_) => return 1,
        };

        if status.contains(git2::Status::WT_MODIFIED) || status.contains(git2::Status::WT_NEW) {
            //println!("add '{}'", path.display());
            0
        } else {
            1
        }
    };
    let cb = if update {
        Some(cb as &mut git2::IndexMatchedPath)
    } else {
        None
    };

    if update {
        index.update_all(files.iter(), cb)?;
    } else {
        index.add_all(files.iter(), git2::IndexAddOption::DEFAULT, cb)?;
    }

    index.write()?;

    Ok(())
}

pub fn git_commit<P>(repo: P, msg: &str) -> Result<(), git2::Error>
where
    P: AsRef<Path>,
{
    let repo = Repository::open(&repo)?;

    let mut index = repo.index()?;
    let oid = index.write_tree()?;
    let signature = repo.signature()?;
    let parent_commit = repo.head()?.peel_to_commit()?;
    let tree = repo.find_tree(oid)?;
    repo.commit(
        Some(git_refs::HEAD),
        &signature,
        &signature,
        msg,
        &tree,
        &[&parent_commit],
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
    let git_repo = PathBuf::new();
    let dot_git = git_repo.join(repo).join(".git");

    dot_git.exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Signature;
    use std::fs;
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
