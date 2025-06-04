use std::fs;
use std::path::PathBuf;

use anyhow;
use clap::{Parser, Subcommand};
use serde_json;
use splines::Spline;

mod analyze;
mod apply;
mod generate;
mod gui;
mod step_description;

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

        #[arg(short, long)]
        invert: bool,
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
        #[arg(short, long)]
        process: Option<String>,
        #[arg(short, long)]
        notes: Option<String>,
    },
    Gui {},
}

fn apply(
    input_pathbuf: &PathBuf,
    curve_pathbuf: &PathBuf,
    output_pathbuf: &PathBuf,
    _debug: bool,
) -> anyhow::Result<()> {
    let input_file_path = fs::canonicalize(&input_pathbuf)?;
    let curve_file_path = fs::canonicalize(&curve_pathbuf)?;
    let output_file_path = fs::canonicalize(&output_pathbuf)?;

    let image = image::open(&input_file_path)?;
    let curve_data = fs::read_to_string(curve_file_path)?;
    let curve = serde_json::from_str::<Spline<f64, f64>>(&curve_data)?;

    let curved_image = apply::apply(&image, &curve);

    curved_image.save(output_file_path)?;
    Ok(())
}

fn analyze(
    input: &PathBuf,
    output_dir: &PathBuf,
    invert_image: bool,
    debug: bool,
) -> anyhow::Result<()> {
    let input_file_path = fs::canonicalize(&input)?;
    let output_dir = fs::canonicalize(&output_dir)?;

    let curve_file = fs::File::create(output_dir.join("curve.json"))?;
    let image = image::open(input_file_path)?;
    let analyze_results = analyze::analyze(&image, invert_image, debug)?;

    serde_json::to_writer(&curve_file, &analyze_results.curve)?;
    Ok(())
}

fn generate(
    output_path: &PathBuf,
    process: Option<String>,
    notes: Option<String>,
) -> anyhow::Result<()> {
    let image = generate::generate(process, notes)?;
    image.save(output_path)?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match &args.command {
        Commands::Analyze {
            input,
            output_dir,
            invert,
        } => {
            analyze(&input, &output_dir, *invert, args.debug)?;
        }
        Commands::Generate {
            process,
            notes,
            output,
        } => {
            generate(output, process.clone(), notes.clone())?;
        }
        Commands::Apply {
            input,
            output,
            curve,
        } => {
            apply(input, curve, output, args.debug)?;
        }
        Commands::Gui {} => {
            gui::start(args.debug);
        }
    }
    Ok(())
}
