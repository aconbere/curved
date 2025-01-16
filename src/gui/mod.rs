use std::fs;
use std::path::PathBuf;

use eframe::egui;
use egui::epaint;
use egui::widgets;
use egui::widgets::Widget;
use egui_extras;
use image;
use image::DynamicImage;
use splines::Spline;

use super::analyze;
use super::generate;
use super::apply;

struct ImageState {
    path: PathBuf,
    image: DynamicImage,
}

enum AnalyzeStates {
    WaitingForImage,
    ImageLoaded,
    Analyzed(analyze::AnalyzeResults),
}

struct ApplyState {
    curve: Option<Spline<f64, f64>>,
    image: Option<DynamicImage>,
    curved_image: Option<DynamicImage>,
}

struct GenerateState {
    process: String,
    notes: String,
    image: Option<DynamicImage>,
}


#[derive(Default)]
enum AppStates {
    #[default]
    GenerateOrAnalyze,
    Analyze(AnalyzeStates),
    Generate(GenerateState),
    Apply(ApplyState),
}

#[derive(Default)]
struct CurvedApp {
    state: AppStates,
    debug: bool,
    image_state: Option<ImageState>,
    preview: Option<TextureBufferedImage>,
}

impl CurvedApp {
    fn new(_cc: &eframe::CreationContext<'_>, debug: bool) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        Self {
            debug,
            ..Self::default()
        }
    }
}

struct TextureBufferedImage {
    texture: Option<egui::TextureHandle>,
    color_image: epaint::ColorImage,
    handle: String,
}

/* TextureBufferedImage is a container for a texture and handle so that we can easily render it.
 * Normally you would use something like egui::image or the image widget except that since
 * we want to use our image.rs processes we want to own the original image bytes and simply
 * present egui with some data to render.
 */
impl TextureBufferedImage {
    fn new(handle: String, image: &DynamicImage) -> Self {
        // Our firs step is to convert from that image.rs dynamic image into a egui color image so
        // that we can send it to the gpu to get a texture handle
        let buffer = image.to_rgba8();
        let pixels = buffer.as_flat_samples();

        let color_image = epaint::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            pixels.as_slice(),
        );

        Self {
            texture: None,
            handle,
            color_image,
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let handle = self.handle.clone();
        let ci = &self.color_image;

        // Careful! This isn't safe to run in immediate mode this generates the texture
        // and memoizes it
        let texture: &egui::TextureHandle = self.texture.get_or_insert_with(|| {
            ui.ctx()
                .load_texture(handle, ci.clone(), Default::default())
        });

        widgets::Image::new((texture.id(), texture.size_vec2()))
            .shrink_to_fit()
            .ui(ui);
    }
}

/* Notes:
 *
 * Store user local state in $XDG_DATA_HOME or $HOME/.local/state
 *
 * Maybe a sqlite database with previously stored curves? Could store them with date, process used,
 * maybe a snapshot of the scan?
 *
 * Maybe store the prevoius state of the app there so restarts are nicer?
 */

pub fn start(debug: bool) {
    println!("starting gui");
    println!("Debug: {}", debug);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([500.0, 700.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Curved",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(CurvedApp::new(cc, debug)))
        }),
    )
    .unwrap();
}

impl eframe::App for CurvedApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.state {
                AppStates::GenerateOrAnalyze => {}
                _ => {
                    if ui.button("Back").clicked() {
                        self.state = AppStates::GenerateOrAnalyze
                    }
                }
            }
            match &mut self.state {
                AppStates::GenerateOrAnalyze => {
                    ui.heading("Curved");
                    ui.separator();
                    ui.add_space(12.0);

                    ui.label("Curved is a tool to help you build tone adjustment curves for analog printing. To use it first generate a step wedge. Then go print the step wedge however you like. Scan the results and use the Analyze function to produce a tone adjustment curve. Finally you can apply the curve to an image with the Apply tool.");
                    ui.add_space(12.0);
                    
                    ui.heading("Generate Step Wedge");
                    ui.separator();
                    ui.label("Use this tool to build a fresh step wedge for your testing.");
                    if ui.button("Generate").clicked() {
                        self.state = AppStates::Generate(GenerateState{
                            process: "".to_string(), notes: "".to_string(), image: None,
                        });
                    }
                    ui.add_space(12.0);

                    ui.heading("Analyze Scan");
                    ui.separator();
                    ui.label("Use this tool build a tone adjustment curve from the scan of your print.");
                    if ui.button("Analyze").clicked() {
                        self.state = AppStates::Analyze(AnalyzeStates::WaitingForImage);
                    }
                    ui.add_space(12.0);

                    ui.heading("Apply Curve");
                    ui.separator();
                    ui.label("Use this tool apply a curve to an image you want to print.");
                    if ui.button("Apply").clicked() {
                        self.state = AppStates::Apply(ApplyStates::WaitingForImageAndCurve(
                            WaitingForImageAndCurveState {
                                curve: None,
                                image: None,
                            },
                        ));
                    }
                }
                AppStates::Generate(ref mut generate_state) => {
                    ui.heading("Generate Step Wedge");
                    ui.label("Generates a step wedge for printing as a transparency. Output will be a 16bit greyscale image. It is generated in its \"inverted\" form and does not need to be further inverted before printing. A 300 dpi print should be about 5\"x5.25\".");

                    // Consider writing these to a label in the image
                    // Maybe store a list of procesess to choose from
                    // and add new ones?
                    ui.label("Information here will be added to the negative so you can keep track of what this negative was for");
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            let process_label = ui.label("Process: ");
                            ui.text_edit_singleline(&mut generate_state.process)
                                .labelled_by(process_label.id);
                        });
                        ui.horizontal(|ui| {
                            let notes_label = ui.label("Notes: ");
                            ui.text_edit_singleline(&mut generate_state.notes)
                                .labelled_by(notes_label.id);
                        });
                    });

                    if ui.button("Generate").clicked() {
                        let image = generate::generate(self.debug);
                        let preview = TextureBufferedImage::new(
                            format!("generated_step_wedge_{}_{}", generate_state.process, generate_state.notes),
                            &image
                        );
                        generate_state.image  = Some(image);
                        self.preview = Some(preview);

                    }

                    if let Some(image) = &generate_state.image {
                        if let Some(preview) = &mut self.preview {
                            preview.ui(ui);
                        }
                        if ui.button("Save").clicked() {
                            if let Some(path) = rfd::FileDialog::new().save_file() {
                                image.save(path).unwrap();
                            }
                        }
                    }
                }
                AppStates::Apply(apply_state) => {
                    ui.heading("Apply Curve");
                    ui.label("This tool will apply a curve generated from Analyze to an input image. Run this on an image in order to prep it for printing.");

                    // Maybe thing about adding an invert option?
                    //
                    if ui.button("Select Image").clicked() {
                        if let Some(image_file) = rfd::FileDialog::new().pick_file() {
                            let image = image::open(&image_file).unwrap();
                            let preview = TextureBufferedImage::new(
                                image_file.clone().into_os_string().into_string().unwrap(),
                                &image,
                            );
                            apply_state.image = Some(image);
                            self.preview = Some(preview);
                        }
                    };

                    if ui.button("select curve").clicked() {
                        if let Some(curve_file) = rfd::FileDialog::new().pick_file() {
                            let curve_data = fs::read_to_string(curve_file).unwrap();
                            let curve =
                                serde_json::from_str::<Spline<f64, f64>>(&curve_data).unwrap();
                            apply_state.curve = Some(curve);
                        }
                    };

                    if let Some(preview) = &mut self.preview {
                        preview.ui(ui);
                    }

                    if let (Some(image), Some(curve)) = (&apply_state.image, &apply_state.curve) {
                        if ui.button("Apply").clicked() {
                            let curved_image = apply::apply(&image, &curve);

                            let preview = TextureBufferedImage::new(
                                "curved_image_preview".to_string(),
                                &curved_image,
                            );
                            self.preview = Some(preview);
                        }
                    }

                    if let Some(curved_image) = apply_state.curved_image {
                        if ui.button("Save").clicked() {
                            if let Some(path) = rfd::FileDialog::new().save_file() {
                                curved_image.save(path).unwrap();
                            }
                        }
                    }
                }
                AppStates::Analyze(analyze_state) => {
                    ui.heading("Analyze Scan");
                    ui.separator();
                    ui.add_space(12.0);

                    ui.label("This tool will evaluate a scan of the generated step wedge to generate a new curve file.");

                    match analyze_state {
                        AnalyzeStates::WaitingForImage => {
                            if ui.button("select scan").clicked() {
                                if let Some(file) = rfd::FileDialog::new().pick_file() {
                                    let path = PathBuf::from(file.display().to_string());
                                    let image = image::open(&path).unwrap();

                                    let preview = TextureBufferedImage::new(
                                        path.clone().into_os_string().into_string().unwrap(),
                                        &image,
                                    );

                                    self.image_state = Some(ImageState {
                                        path: path.clone(),
                                        image,
                                    });
                                    self.preview = Some(preview);

                                    self.state = AppStates::Analyze(AnalyzeStates::ImageLoaded);
                                }
                            }
                        }
                        AnalyzeStates::ImageLoaded => {
                            if let Some(image_state) = &mut self.image_state {
                                if ui.button("analyze").clicked() {
                                    let analyze_results =
                                        analyze::analyze(&image_state.image, self.debug).unwrap();

                                    self.state =
                                        AppStates::Analyze(AnalyzeStates::Analyzed(analyze_results));
                                }

                                ui.horizontal(|ui| {
                                    ui.label("Picked file:");
                                    ui.monospace(image_state.path.to_string_lossy());
                                });
                                if let Some(preview) = &mut self.preview {
                                    preview.ui(ui)
                                }
                            }
                        }
                        AnalyzeStates::Analyzed(analyze_results) => {
                            if ui.button("save curve").clicked() {
                                if let Some(path) = rfd::FileDialog::new().save_file() {
                                    let curve_file = fs::File::create(path).unwrap();
                                    serde_json::to_writer(&curve_file, &analyze_results.curve).unwrap();
                                }
                            }
                        }
                    }
                },
            }
        });
    }
}
