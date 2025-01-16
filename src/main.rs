use std::fs;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use image::Luma;
use imageproc::map::map_pixels;
use serde_json;
use splines::Spline;

mod analyze;
mod generate;
mod gui;

#[derive(Parser, Debug)]
#[command()]
struct Args {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long)]
    debug: bool,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Adds files to myapp
    Analyze {
        #[arg(short, long)]
        input: PathBuf,

        #[arg(short, long)]
        output_dir: PathBuf,
    },
    Apply {
        #[arg(short, long)]
        input: PathBuf,

        #[arg(short, long)]
        curve: PathBuf,

        #[arg(short, long)]
        output: PathBuf,
    },
    Generate {
        #[arg(short, long)]
        output: PathBuf,
    },
    Gui {},
}

fn apply(input_pathbuf: &PathBuf, curve_pathbuf: &PathBuf, output_pathbuf: &PathBuf, _debug: bool) {
    let input_file_path = fs::canonicalize(&input_pathbuf).unwrap();
    let curve_file_path = fs::canonicalize(&curve_pathbuf).unwrap();

    let output_file_path = fs::canonicalize(&output_pathbuf).unwrap();

    let input_image = image::open(&input_file_path).unwrap();
    let input_image_16 = input_image.to_luma16();

    let curve_data = fs::read_to_string(curve_file_path).unwrap();
    let curve = serde_json::from_str::<Spline<f64, f64>>(&curve_data).unwrap();

    let curved_image = map_pixels(&input_image_16, |_x, _y, p| {
        Luma([curve.clamped_sample(p[0] as f64).unwrap() as u16])
    });

    curved_image.save(output_file_path).unwrap();
}

fn analyze(input: &PathBuf, output_dir: &PathBuf, debug: bool) {
    let input_file_path = fs::canonicalize(&input).unwrap();
    let output_dir = fs::canonicalize(&output_dir).unwrap();

    let curve_file = fs::File::create(output_dir.join("curve.json")).unwrap();
    let image = image::open(input_file_path).unwrap();
    let analyze_results = analyze::analyze(&image, debug).unwrap();

    serde_json::to_writer(&curve_file, &analyze_results.curve).unwrap();
}

fn generate(output_path: &PathBuf, debug: bool) {
    let image = generate::generate(debug);
    image.save(output_path).unwrap();
}

fn main() {
    let args = Args::parse();

    match &args.command {
        Commands::Analyze { input, output_dir } => {
            analyze(&input, &output_dir, args.debug);
        }
        Commands::Generate { output } => {
            generate(output, args.debug);
        }
        Commands::Apply {
            input,
            output,
            curve,
        } => {
            apply(input, curve, output, args.debug);
        }
        Commands::Gui {} => {
            gui::start(args.debug);
        }
    }
}
