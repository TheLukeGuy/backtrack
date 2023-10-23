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

use anyhow::{Context, Result};
use backtrack_test_executor::args::Args;
use clap::Parser;
use log::{debug, LevelFilter};
use simplelog::{ColorChoice, ConfigBuilder, TermLogger, TerminalMode};

fn main() -> Result<()> {
    let args = Args::parse();
    init_logging().context("failed to initialize logging")?;
    debug!("Parsed args: {args:?}");
    backtrack_test_executor::run(args)
}

fn init_logging() -> Result<()> {
    let level = if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    TermLogger::init(
        level,
        ConfigBuilder::new()
            .set_time_level(LevelFilter::Off)
            .set_thread_level(LevelFilter::Off)
            .set_target_level(LevelFilter::Off)
            .build(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )?;
    Ok(())
}
