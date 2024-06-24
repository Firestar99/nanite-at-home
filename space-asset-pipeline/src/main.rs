use clap::Parser;
use smol::future::block_on;
use space_asset_pipeline::meshlet::process::Gltf;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
	/// The path from which to load the gltf model
	path: PathBuf,

	/// The output path
	#[arg(short, long)]
	out: Option<PathBuf>,

	/// Whether to print the debug of the output structs
	#[arg(long)]
	debug_verbose: bool,

	/// The amount of threads to use
	#[arg(long, short = 'j')]
	threads: Option<usize>,
}

fn main() -> Result<(), Box<dyn Error>> {
	let args = Args::parse();
	std::env::set_var(
		"SMOL_THREADS",
		args.threads
			.unwrap_or_else(|| std::thread::available_parallelism().map(NonZeroUsize::get).unwrap_or(1))
			.to_string(),
	);

	let gltf = Gltf::open(PathBuf::from(args.path))?;
	let scene = block_on(gltf.process())?;

	if let Some(out) = args.out {
		let vec = rkyv::to_bytes::<_, 1024>(&scene)?;
		File::create(&out)?.write_all(&vec)?;

		let out_zstd = out
			.parent()
			.unwrap_or(Path::new("."))
			.join(format!("{}.zstd", out.file_name().unwrap().to_str().unwrap()));
		zstd::stream::write::Encoder::new(File::create(out_zstd)?, 0)?
			.auto_finish()
			.write_all(&vec)?;
	}

	if args.debug_verbose {
		println!("{:#?}", scene);
	}

	Ok(())
}
