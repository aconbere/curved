use std::fmt;
use std::fs::File;
use std::path::PathBuf;

use clap::Parser;

use tiff::decoder::{Decoder, DecodingResult};
use tiff::ColorType;

fn colortype_to_str(c: ColorType) -> String {
    return match c {
        ColorType::RGB(depth) => format!("RGB: {}", depth),
        ColorType::Gray(depth) => format!("Gray:{}", depth),
        ColorType::Palette(depth) => format!("Palette:{}", depth),
        ColorType::GrayA(depth) => format!("GrayA:{}", depth),
        ColorType::RGBA(depth) => format!("RGBA:{}", depth),
        ColorType::CMYK(depth) => format!("CMYK:{}", depth),
        ColorType::YCbCr(depth) => format!("YCbCr:{}", depth),
    };
}

struct Pos {
    x: u32,
    y: u32,
}

impl Pos {
    fn new(x: u32, y: u32) -> Pos {
        Pos { x, y }
    }
}

impl fmt::Display for Pos {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

struct Image {
    pixels: Vec<u16>,
    width: u32,
    height: u32,
}

impl Image {
    fn new(pixels: Vec<u16>, width: u32, height: u32) -> Image {
        return Image {
            pixels,
            width,
            height,
        };
    }

    fn index_from_pos(&self, pos: &Pos) -> usize {
        assert!(
            pos.x <= self.width,
            "Assertion! x: {}, width: {}",
            pos.x,
            self.width
        );
        assert!(
            pos.y <= self.height,
            "Assertion! y: {}, height: {}",
            pos.y,
            self.height
        );
        ((pos.y * self.width) + pos.x).try_into().unwrap()
    }

    fn sample_at(&self, pos: Pos, width: u32, height: u32) -> Result<Sample, String> {
        let mut pixels = Vec::new();

        for y in pos.y..(pos.y + height) {
            for x in pos.x..(pos.x + width) {
                let idx = self.index_from_pos(&Pos::new(x, y));
                pixels.push(self.pixels[idx]);
            }
        }

        Sample::new(pixels)
    }
}

struct Sample {
    pixels: Vec<u16>,
}

impl fmt::Display for Sample {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, x) in self.pixels.iter().enumerate() {
            write!(f, "({}, {}) \n", i, x)?;
        }
        write!(f, "\n")
    }
}

struct Stat {
    count: u32,
    mean: u16,
    min: u16,
    max: u16,
}

impl Sample {
    fn new(pixels: Vec<u16>) -> Result<Sample, String> {
        if pixels.len() <= 0 {
            Err("pixels empty".to_string())
        } else {
            Ok(Sample { pixels })
        }
    }

    fn statistics(&self) -> Stat {
        let count = self.pixels.len() as u32;

        if count == 0 {
            panic!("pixels should always have count > 0");
        }

        let mut total: u32 = 0;
        let mut min: u16 = u16::MAX;
        let mut max: u16 = 0;

        for x in self.pixels.iter() {
            let _x = *x;
            total = total + (_x as u32);
            if _x < min {
                min = _x;
            }
            if _x > max {
                max = _x;
            }
        }

        Stat {
            count,
            mean: (total / count) as u16,
            min,
            max,
        }
    }
}

#[derive(Parser, Debug)]
#[command()]
struct Args {
    #[arg(short, long)]
    file_path: PathBuf,
}

fn main() {
    let args = Args::parse();

    println!("File Path: {}", args.file_path.display());
    let tiff_file = File::open(args.file_path).unwrap();

    let mut decoder = Decoder::new(tiff_file).expect("Failed to create decoder");
    let (width, height) = decoder.dimensions().expect("Failed to decode dimensions");

    println!("Width: {}", width);
    println!("Height: {}", height);

    let colortype = decoder.colortype().expect("Failed to decode colortype");
    println!("ColorType: {}", colortype_to_str(colortype));

    let decoding_result = decoder.read_image().expect("Failed to decode tiff");

    let image_data = match decoding_result {
        DecodingResult::U16(res) => res,
        _ => panic!("wrong bitdepth"),
    };

    let image = Image::new(image_data, width, height);

    // test index_from_pos
    // let pos = Pos::new(5, 5);
    // let idx = image.index_from_pos(&pos);
    // println!("pos: {}, idx: {}", pos, idx);

    // 1. What is "Black"
    //
    // Assume area around the step wedge represents max black for the print.
    // Take a sample of those pixels. This is our "black point".
    //
    // Is it valuable to know the range of expected blacks so that I can ask
    // questions like "Is this within 1 std deviation of black"?
    //
    // 2: What is "White"
    //
    // Assume the whitest square is the first square. First job is to find
    // the first square.
    //
    // The first square should be the first "substantial" patch of low values
    // found when search from left to right and top to bottom.
    //
    // It should be found in the first 15% of the image width.
    let edge_sample = image.sample_at(Pos::new(10, 10), 10, 10).unwrap();
    let edge_stats = edge_sample.statistics();
    println!("edge:");
    println!("\tcount: {}", edge_stats.count);
    println!("\tmean: {}", edge_stats.mean);
    println!("\tmin: {}", edge_stats.min);
    println!("\tmax: {}", edge_stats.max);

    // sample at known white area
    let sample = image.sample_at(Pos::new(180, 80), 10, 10).unwrap();
    let stats = sample.statistics();
    println!("deep:");
    println!("\tcount: {}", stats.count);
    println!("\tmean: {}", stats.mean);
    println!("\tmin: {}", stats.min);
    println!("\tmax: {}", stats.max);
    // println!("sample: {}", sample);
}
