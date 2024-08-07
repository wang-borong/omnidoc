use git2::Repository;
use std::path::Path;

pub fn git_clone<P>(url: &str, p: P)
    where P: AsRef<Path> {

    let _repo = match Repository::clone(url, p) {
        Ok(repo) => repo,
        Err(e) => panic!("failed to clone {}", e),
    };
}

pub fn git_init<P>(p: P)
    where P: AsRef<Path> {
    let _repo = match Repository::init(p) {
        Ok(repo) => repo,
        Err(e) => panic!("failed to init {}", e),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_clone() {
        git_clone("https://github.com/wang-borong/embedded-knowledge", "embedded-knowledge");
        assert_eq!(Path::new("embedded-knowledge").exists(), true);
    }

    #[test]
    fn test_git_init() {
        git_init("test_git_init");
        assert_eq!(Path::new("test_git_init").exists(), true);
    }
}
