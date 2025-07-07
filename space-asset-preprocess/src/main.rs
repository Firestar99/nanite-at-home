use clap::Parser;
use space_asset_preprocess::meshlet::build_script::build_script;
use std::ops::Deref;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct PreprocessArgs {
	#[arg(short, long)]
	models_dir: PathBuf,
	#[arg(short, long)]
	out_dir: PathBuf,
	#[arg(long)]
	models_rs: Option<PathBuf>,
}

pub fn main() -> anyhow::Result<()> {
	let args = PreprocessArgs::parse();
	let result = build_script(
		args.models_dir.deref(),
		args.out_dir.deref(),
		args.models_rs.as_deref(),
		false,
	)?;
	println!("{result:#?}");
	Ok(())
}
