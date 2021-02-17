mod cargo;
mod errors;
mod infer;
mod options;
mod steps;

use std::collections::HashSet;
use std::iter::Iterator;
use std::process::{Command, Stdio};
use std::vec::Vec;

use crate::errors::CargoPlayError;
use crate::options::Options;
use crate::steps::*;

fn main() -> Result<(), CargoPlayError> {
    let args = std::env::args().collect::<Vec<_>>();
    let opt = Options::parse(args);
    if opt.is_err() {
        return Ok(());
    }
    let opt = opt.unwrap();

    let src_hash = opt.src_hash();
    let package_name = format!("p{}", src_hash);
    let temp = temp_dir(opt.temp_dirname());

    if opt.cached && temp.exists() {
        let mut bin_path = temp.join("target");
        if opt.release {
            bin_path.push("release");
        } else {
            bin_path.push("debug");
        }
        // TODO reuse logic to formulate package name, i.e. to_lowercase
        bin_path.push(&package_name.to_lowercase());
        if bin_path.exists() {
            let mut cmd = Command::new(bin_path);
            return cmd
                .args(opt.args)
                .stderr(Stdio::inherit())
                .stdout(Stdio::inherit())
                .status()
                .map(|_| ())
                .map_err(CargoPlayError::from);
        }
    }

    let files = read_inputs(&opt.src)?;
    let contents: Vec<&str> = files.iter().map(|(content, _)| content.as_ref()).collect();
    let dependencies = extract_headers(&contents);

    let infers = if opt.infer {
        infer::analyze_sources(&contents)?
    } else {
        HashSet::new()
    };

    if opt.clean {
        rmtemp(&temp);
    }
    mktemp(&temp);
    write_cargo_toml(
        &temp,
        package_name,
        dependencies,
        opt.edition.clone(),
        infers,
    )?;
    copy_sources(&temp, &files)?;

    let end = if let Some(save) = opt.save {
        copy_project(&temp, &save)?
    } else {
        run_cargo_build(&opt, &temp)?
    };

    match end.code() {
        Some(code) => std::process::exit(code),
        None => std::process::exit(-1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_headers() {
        let inputs: Vec<&str> = vec![
            r#"//# line 1
//# line 2
// line 3
//# line 4"#,
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        let result = extract_headers(&inputs);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], String::from("line 1"));
        assert_eq!(result[1], String::from("line 2"));
    }
}
