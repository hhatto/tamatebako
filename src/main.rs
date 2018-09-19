#[macro_use]
extern crate structopt;
#[macro_use]
extern crate serde_derive;
extern crate toml;

use std::path::PathBuf;
use structopt::StructOpt;

mod config;

#[derive(Debug, StructOpt)]
#[structopt(name="tamatebako", about="new version checker for OSS Projects")]
struct CLIOption {
    #[structopt(long="verbose", help="verbose output")]
    verbose: bool,
    #[structopt(short="c", long="config", help="config file", parse(from_os_str))]
    config_file: PathBuf,
}

fn main() {
    let opts = CLIOption::from_args();
    let config_filepath = opts.config_file;
    let config = config::load_config(config_filepath.to_str().expect("fail to get config filename"));
    println!("config: {:?}", config);
}
