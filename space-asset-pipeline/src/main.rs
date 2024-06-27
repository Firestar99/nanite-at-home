use clap::Parser;
use futures::executor::block_on;
use space_asset_pipeline::meshlet::process::Gltf;
use std::error::Error;
use std::fs::File;
use std::io::Write;
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
fn inner_main() -> Result<(), Box<dyn Error>> {
	let args = Args::parse();
	rayon::ThreadPoolBuilder::new()
		.num_threads(args.threads.unwrap_or(0))
		.thread_name(|id| format!("Rayon-{}", id))
		.build_global()
		.unwrap();

	let gltf = Gltf::open(PathBuf::from(args.path))?;
	let scene = block_on(gltf.process())?;

	if let Some(out) = args.out {
		let vec = {
			profiling::scope!("serializing");
			rkyv::to_bytes::<_, 1024>(&scene)?
		};

		{
			profiling::scope!("write uncompressed");
			File::create(&out)?.write_all(&vec)?;
		}

		{
			profiling::scope!("zstd stream to file");
			let out_zstd = out
				.parent()
				.unwrap_or(Path::new("."))
				.join(format!("{}.zstd", out.file_name().unwrap().to_str().unwrap()));
			zstd::stream::write::Encoder::new(File::create(out_zstd)?, 0)?
				.auto_finish()
				.write_all(&vec)?;
		}

		let zstd = {
			profiling::scope!("zstd bulk");
			zstd::bulk::compress(&vec, 0).unwrap()
		};

		{
			profiling::scope!("write zstd");
			let out_zstd2 = out
				.parent()
				.unwrap_or(Path::new("."))
				.join(format!("{}2.zstd", out.file_name().unwrap().to_str().unwrap()));
			File::create(&out_zstd2)?.write_all(&zstd)?;
		}
	}

	if args.debug_verbose {
		profiling::scope!("debug_verbose");
		println!("{:#?}", scene);
	}

	Ok(())
}
