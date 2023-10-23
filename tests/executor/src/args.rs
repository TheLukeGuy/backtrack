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
    fmt::{self, Display, Formatter, Write},
    path::PathBuf,
    str::FromStr,
};

use anyhow::{Error, Result};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
pub struct Args {
    /// The directory to store intermediate files in
    #[arg(long, default_value_os_t = tests_child("run"))]
    pub run_dir: PathBuf,
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Extract all Typst compilers from the specified archive
    ExtractCompilers {
        #[arg(default_value_os_t = tests_child("compilers.7z"))]
        archive: PathBuf,
    },
    /// Generate reference documents for the specified Typst compilers
    GenRefs(#[command(flatten)] CompileArgs),
    /// Test the specified Typst compilers
    Test(#[command(flatten)] CompileArgs),
}

#[derive(Debug, clap::Args)]
pub struct CompileArgs {
    /// A comma-separated list of names of the Typst compilers to use, or `*` to
    /// use all available compilers
    #[arg(default_value_t)]
    pub compilers: CompilersSpec,
    /// The sample Typst source file
    #[arg(long, default_value_os_t = tests_child("sample.typ"))]
    pub sample: PathBuf,
    /// The directory that contains (or will contain) reference documents
    #[arg(long, default_value_os_t = tests_child("refs"))]
    pub ref_dir: PathBuf,
    /// The project root to pass to Typst
    #[arg(long, default_value_os = ".")]
    pub project_root: PathBuf,
}

#[derive(Clone, Debug, Default)]
pub enum CompilersSpec {
    #[default]
    All,
    Specific(Vec<String>),
}

impl Display for CompilersSpec {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::All => f.write_char('*'),
            Self::Specific(ids) => {
                let joined = ids.join(",");
                f.write_str(&joined)
            }
        }
    }
}

impl FromStr for CompilersSpec {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        if s == "*" {
            Ok(Self::All)
        } else {
            let ids = s.split(',').map(ToOwned::to_owned).collect();
            Ok(Self::Specific(ids))
        }
    }
}

fn tests_child(name: &str) -> PathBuf {
    ["tests", name].iter().collect()
}
