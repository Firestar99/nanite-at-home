use clap::Parser;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
	#[clap(long, value_parser)]
	pub gpu: Option<String>,
	#[clap(long, action)]
	pub validation_layer: bool,
	#[clap(long, action)]
	pub renderdoc: bool,
}
