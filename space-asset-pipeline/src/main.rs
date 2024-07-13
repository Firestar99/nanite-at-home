use anyhow::Context;
use clap::Parser;
use space_asset_pipeline::gltf::Gltf;
use space_asset_pipeline::meshlet::process::process_meshlets;
use std::fs;
use std::fs::File;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
	/// The path from which to load the gltf model
	path: PathBuf,

	/// The output path
	#[arg(short, long)]
	out: PathBuf,

	/// Whether to print the debug of the output structs
	#[arg(long)]
	debug_verbose: bool,

	/// The amount of threads to use
	#[arg(long, short = 'j')]
	threads: Option<usize>,
}

fn main() -> anyhow::Result<()> {
	#[cfg(feature = "profile-with-puffin")]
	let _puffin_server = {
		profiling::puffin::set_scopes_on(true);
		let server_addr = format!("127.0.0.1:{}", puffin_http::DEFAULT_PORT);
		puffin_http::Server::new(&server_addr).unwrap()
	};

	let result = inner_main();
	profiling::finish_frame!();
	result
}

#[profiling::function]
fn inner_main() -> anyhow::Result<()> {
	let args = Args::parse();
	rayon::ThreadPoolBuilder::new()
		.num_threads(args.threads.unwrap_or(0))
		.thread_name(|id| format!("Rayon-{}", id))
		.build_global()
		.unwrap();

	let gltf = Gltf::open(&args.path).with_context(|| format!("opening gltf file failed {:?}", args.path))?;
	let scene = process_meshlets(&gltf).with_context(|| format!("processing gltf failed {:?}", args.path))?;

	{
		fs::create_dir_all(args.out.parent().unwrap())
			.with_context(|| format!("failed creating output directories for file {:?}", args.out))?;
		let out_file =
			File::create(&args.out).with_context(|| format!("failed creating output file {:?}", args.out))?;
		scene
			.serialize_to(out_file)
			.with_context(|| format!("zstd stream failed writing {:?}", args.out))?;
	}

	if args.debug_verbose {
		profiling::scope!("debug_verbose");
		println!("{:#?}", scene);
	}

	Ok(())
}
