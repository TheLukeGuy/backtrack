// Copyright © 2023 Luke Chambers
// This file is part of Backtrack.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at <http://www.apache.org/licenses/LICENSE-2.0>.
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
// WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the
// License for the specific language governing permissions and limitations under
// the License.

use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::{self, Display, Formatter},
    fs, io,
    path::{Path, PathBuf},
    process::{self, Command, Output},
    result, str,
};

use anyhow::{bail, Context, Result};
use args::{Args, Cmd, CompilersSpec};
use log::{debug, error, info, warn};
use owo_colors::{OwoColorize, Style};
use terminal_size::{terminal_size, Width};
use thiserror::Error;

pub mod args;

pub fn run(args: Args) -> Result<()> {
    fs::create_dir_all(&args.run_dir)
        .context("failed to create the run directory")?;
    let (op, compile_args, cmp_dir) = match args.cmd {
        Cmd::ExtractCompilers { archive } => {
            return extract_compilers(archive, &args.run_dir)
                .context("failed to extract the compilers")
        }
        Cmd::GenRefs(compile_args) => {
            fs::create_dir_all(&compile_args.ref_dir)
                .context("failed to create the reference directory")?;
            (Op::GenRefs, compile_args, None)
        }
        Cmd::Test(compile_args) => {
            let cmp_dir = args.run_dir.join("cmps");
            fs::create_dir_all(&cmp_dir)
                .context("failed to create the compare directory")?;
            (Op::Test, compile_args, Some(cmp_dir))
        }
    };

    let compiler_dir = args.run_dir.join("compilers");
    let compilers: Vec<_> = match compile_args.compilers {
        CompilersSpec::All => fs::read_dir(&compiler_dir)
            .context("failed to read the compiler directory")?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                entry.file_name().into_string().ok().and_then(|name| {
                    if name.starts_with('v') {
                        let compiler = Compiler::new(
                            name,
                            entry.path(),
                            &compile_args.ref_dir,
                            cmp_dir.as_ref(),
                        );
                        Some(compiler)
                    } else {
                        None
                    }
                })
            })
            .collect(),
        CompilersSpec::Specific(names) => names
            .into_iter()
            .map(|name| {
                let path = compiler_dir.join(&name);
                Compiler::new(
                    name,
                    path,
                    &compile_args.ref_dir,
                    cmp_dir.as_ref(),
                )
            })
            .collect(),
    };
    debug!("Collected compilers: {compilers:?}");

    let separator_width = if let Some((Width(width), _)) = terminal_size() {
        width - 7
    } else {
        80
    };
    let separator = "-".repeat(separator_width.into());

    let mut results = HashMap::with_capacity(compilers.len());
    let mut success = true;
    let mut mismatches = false;
    let mut longest_name_len = 0;
    for compiler in compilers {
        info!("{separator}");

        let name_len = compiler.name.len();
        if name_len > longest_name_len {
            longest_name_len = name_len;
        }

        info!("Running {op:?} for {compiler}.");
        if let Err(err) = compiler.set_executable() {
            error!("Failed to set the executable's permissions: {err}");
            results.insert(
                compiler.into_name(),
                OpResult::Err("permission setting"),
            );
            success = false;
            continue;
        }
        if let Ok(version) = compiler.reported_version() {
            info!("The compiler reports itself as \"{version}\".");
        } else {
            warn!(concat!(
                "Failed to get the compiler version.",
                " This is probably just an old (pre-3/21) compiler.",
            ));
        }

        let result = match op {
            Op::GenRefs => {
                let result = compiler
                    .gen_ref(&compile_args.sample, &compile_args.project_root);
                if let Err(err) = result {
                    error!("Failed to generate the reference document: {err}");
                    err.log_unsuccessful_exit();
                    success = false;
                    OpResult::Err("reference generation")
                } else {
                    info!("Successfully generated the reference document.");
                    OpResult::Ok
                }
            }
            Op::Test => {
                let result = compiler
                    .test(&compile_args.sample, &compile_args.project_root);
                match result {
                    Ok(_) => {
                        info!("The test passed.");
                        OpResult::Ok
                    }
                    Err(TestError::CompileFailed(err)) => {
                        error!("Failed to compile the compare document: {err}");
                        err.log_unsuccessful_exit();
                        success = false;
                        OpResult::Err("compare compilation")
                    }
                    Err(TestError::Mismatch { ref_digest, cmp_digest }) => {
                        error!("The test failed.",);
                        error!("Reference digest: {ref_digest}");
                        error!("Compare digest: {cmp_digest}");
                        success = false;
                        mismatches = true;
                        OpResult::Mismatch
                    }
                    Err(err) => {
                        error!("Failed to run the test: {err}");
                        success = false;
                        OpResult::Err("test")
                    }
                }
            }
        };
        results.insert(compiler.into_name(), result);
    }

    info!("{separator}");
    if success {
        info!("{}", "TEST SUCCESS".bright_green().bold());
    } else {
        info!("{}", "TEST FAILURE".bright_red().bold());
    };
    info!("{separator}");
    for (name, result) in results {
        let padded_name = format!("{name:longest_name_len$}");
        let (result_desc, result_style) = result.fmt();
        info!("{padded_name} | {}", result_desc.style(result_style));
    }
    if mismatches {
        info!(
            "You can find the compiled documents from the failed tests in {}.",
            cmp_dir.unwrap().display(),
        );
    }

    if !success {
        process::exit(1);
    }
    Ok(())
}

fn extract_compilers<F, T>(from: F, to: T) -> Result<()>
where
    F: AsRef<Path>,
    T: AsRef<Path>,
{
    info!("Extracting compilers.");
    sevenz_rust::decompress_file(from, to)
        .context("failed to decompress the archive")?;
    info!("Done! :)");
    Ok(())
}

#[derive(Debug)]
enum Op {
    GenRefs,
    Test,
}

enum OpResult {
    Ok,
    Err(&'static str),
    Mismatch,
}

impl OpResult {
    pub fn fmt(&self) -> (Cow<'static, str>, Style) {
        match self {
            Self::Ok => (Cow::Borrowed("OK"), Style::new().bright_green()),
            Self::Err(stage) => (
                Cow::Owned(format!("Error during {stage}")),
                Style::new().red(),
            ),
            Self::Mismatch => {
                (Cow::Borrowed("Mismatch"), Style::new().bright_red())
            }
        }
    }
}

#[derive(Debug)]
struct Compiler {
    name: String,
    path: PathBuf,
    ref_path: PathBuf,
    cmp_path: Option<PathBuf>,
    arg_layout: CompilerArgLayout,
}

impl Compiler {
    pub fn new<R, C>(
        name: String,
        path: PathBuf,
        ref_dir: R,
        cmp_dir: Option<C>,
    ) -> Self
    where
        R: AsRef<Path>,
        C: AsRef<Path>,
    {
        let pdf = Path::new(&name).with_extension("pdf");
        let arg_layout = CompilerArgLayout::from_compiler_name(&name);
        Self {
            name,
            path,
            ref_path: ref_dir.as_ref().join(&pdf),
            cmp_path: cmp_dir.map(|cmp_dir| cmp_dir.as_ref().join(&pdf)),
            arg_layout,
        }
    }

    pub fn into_name(self) -> String {
        self.name
    }

    #[cfg(unix)]
    pub fn set_executable(&self) -> Result<()> {
        let mut perms = fs::metadata(&self.path)
            .context("failed to get the executable metadata")?
            .permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 755);
        fs::set_permissions(&self.path, perms)
            .context("failed to set the executable's permissions")
    }

    #[cfg(not(unix))]
    pub fn set_executable(&self) -> Result<()> {
        Ok(())
    }

    pub fn reported_version(&self) -> Result<String> {
        let output = self
            .run(|cmd| cmd.arg("--version"))
            .context("failed to run the version command")?;
        if output.status.success() {
            let version = str::from_utf8(&output.stdout)
                .context("the version output isn't a valid UTF-8 string")?
                .trim()
                .to_owned();
            Ok(version)
        } else {
            bail!(
                "the version command exited unsuccessfully (code: {:?})",
                output.status.code(),
            );
        }
    }

    pub fn gen_ref<S, R>(
        &self,
        sample: S,
        project_root: R,
    ) -> result::Result<(), CompileError>
    where
        S: AsRef<Path>,
        R: AsRef<Path>,
    {
        self.compile(sample, &self.ref_path, project_root)
    }

    pub fn test<S, R>(
        &self,
        sample: S,
        project_root: R,
    ) -> result::Result<(), TestError>
    where
        S: AsRef<Path>,
        R: AsRef<Path>,
    {
        let cmp_path =
            self.cmp_path.as_ref().expect("this compiler has no compare path");
        self.compile(sample, cmp_path, project_root)
            .map_err(TestError::from)?;
        let ref_doc =
            fs::read(&self.ref_path).map_err(TestError::RefReadFailed)?;
        let cmp_doc = fs::read(cmp_path).map_err(TestError::CmpReadFailed)?;

        let ref_digest = sha256::digest(ref_doc);
        let cmp_digest = sha256::digest(cmp_doc);
        if ref_digest == cmp_digest {
            Ok(())
        } else {
            Err(TestError::Mismatch { ref_digest, cmp_digest })
        }
    }

    fn compile<I, O, R>(
        &self,
        input: I,
        output: O,
        project_root: R,
    ) -> result::Result<(), CompileError>
    where
        I: AsRef<Path>,
        O: AsRef<Path>,
        R: AsRef<Path>,
    {
        let output = self
            .run(|cmd| {
                self.arg_layout
                    .cfg_cmd(cmd, project_root)
                    .arg(input.as_ref())
                    .arg(output.as_ref())
            })
            .map_err(CompileError::from)?;
        if output.status.success() {
            Ok(())
        } else {
            Err(CompileError::UnsuccessfulExit {
                code: output.status.code(),
                output,
            })
        }
    }

    fn run<C>(&self, cfg_cmd: C) -> io::Result<Output>
    where
        C: FnOnce(&mut Command) -> &mut Command,
    {
        let mut cmd = Command::new(&self.path);
        cfg_cmd(&mut cmd).output()
    }
}

impl Display for Compiler {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.write_str(&self.name)
    }
}

#[derive(Debug)]
enum CompilerArgLayout {
    // 0.7.0..
    SubcommandBeforeRoot,
    // 0.1.0..0.7.0
    SubcommandAfterRoot,
    // ..0.1.0
    NoSubcommand,
}

impl CompilerArgLayout {
    pub fn from_compiler_name(name: &str) -> Self {
        if name.starts_with("v2023-0") {
            return Self::NoSubcommand;
        }
        match name.split_once("v0-") {
            None => Self::SubcommandBeforeRoot,
            Some((_, version)) => {
                let after_root = version
                    .split('-')
                    .next()
                    .and_then(|minor| {
                        minor.parse::<u8>().ok().map(|minor| minor < 7)
                    })
                    .unwrap_or(false);
                if after_root {
                    Self::SubcommandAfterRoot
                } else {
                    Self::SubcommandBeforeRoot
                }
            }
        }
    }

    pub fn cfg_cmd<'a>(
        &self,
        cmd: &'a mut Command,
        project_root: impl AsRef<Path>,
    ) -> &'a mut Command {
        match self {
            Self::SubcommandBeforeRoot => {
                cmd.arg("compile").arg("--root").arg(project_root.as_ref())
            }
            Self::SubcommandAfterRoot => {
                cmd.arg("--root").arg(project_root.as_ref()).arg("compile")
            }
            Self::NoSubcommand => cmd.arg("--root").arg(project_root.as_ref()),
        }
    }
}

#[derive(Debug, Error)]
enum TestError {
    #[error("failed to compile the compare document")]
    CompileFailed(#[from] CompileError),
    #[error("failed to read the reference document")]
    RefReadFailed(io::Error),
    #[error("failed to read the compile document")]
    CmpReadFailed(io::Error),
    #[error("the documents don't match: {ref_digest} vs. {cmp_digest}")]
    Mismatch { ref_digest: String, cmp_digest: String },
}

#[derive(Debug, Error)]
enum CompileError {
    #[error("failed to run the compile command")]
    IoError(#[from] io::Error),
    #[error("the compile command exited unsuccessfully (code: {code:?})")]
    UnsuccessfulExit { code: Option<i32>, output: Output },
}

impl CompileError {
    pub fn log_unsuccessful_exit(&self) {
        if let Self::UnsuccessfulExit { code, output } = self {
            error!(
                concat!(
                    "The compiler exited with a code of {:?}.",
                    " It wrote the following to stderr:",
                ),
                code,
            );
            let output = String::from_utf8_lossy(&output.stderr);
            for line in output.lines() {
                error!("> {line}");
            }
        }
    }
}
