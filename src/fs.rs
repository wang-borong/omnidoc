pub use std::fs::*;

use dirs::data_local_dir;
use std::fs;
use std::path::{Path, PathBuf};

/**
 * copy_dir - copy directory
 * @from: copy from directory
 * @to:   copy to directory
 */
pub fn copy_dir<U: AsRef<Path>, V: AsRef<Path>>(from: U, to: V) -> Result<(), std::io::Error> {
    let mut stack = Vec::new();
    stack.push(PathBuf::from(from.as_ref()));

    let output_root = PathBuf::from(to.as_ref());
    let input_root = PathBuf::from(from.as_ref()).components().count();

    while let Some(working_path) = stack.pop() {
        //println!("process: {:?}", &working_path);

        // Generate a relative path
        let src: PathBuf = working_path.components().skip(input_root).collect();

        // Create a destination if missing
        let dest = if src.components().count() == 0 {
            output_root.clone()
        } else {
            output_root.join(&src)
        };
        if fs::metadata(&dest).is_err() {
            //println!(" mkdir: {:?}", dest);
            fs::create_dir_all(&dest)?;
        }

        for entry in fs::read_dir(working_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                match path.file_name() {
                    Some(filename) => {
                        let dest_path = dest.join(filename);
                        //println!("  copy: {:?} -> {:?}", &path, &dest_path);
                        fs::copy(&path, &dest_path)?;
                    }
                    None => {
                        eprintln!("No such file '{}'", path.display());
                    }
                }
            }
        }
    }

    Ok(())
}

/**
 * copy_from_lib - copy a file or a directory from omnidoc lib
 * @from: relative to omnidoc lib
 * @to:   destination path
 */
pub fn copy_from_lib<U: AsRef<Path>, V: AsRef<Path>>(from: U, to: V) -> Result<(), std::io::Error> {
    let local_data_dir = data_local_dir().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "data_local_dir not found"))?;
    let omnidoc_lib = local_data_dir.join("omnidoc");
    let to_copy = omnidoc_lib.join(from);

    if to_copy.is_dir() {
        copy_dir(to_copy, to)?;
    } else {
        fs::copy(to_copy, to)?;
    }

    Ok(())
}
