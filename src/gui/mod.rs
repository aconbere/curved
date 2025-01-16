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

struct ImageState {
    path: PathBuf,
    image: DynamicImage,
}

enum AnalyzeStates {
    WaitingForImage,
    ImageLoaded,
    Analyzed(analyze::AnalyzeResults),
}

enum ApplyStates {
    WaitingForImageAndCurve(WaitingForImageAndCurveState),
    Applied(AppliedState),
}

struct WaitingForImageAndCurveState {
    curve: Option<Spline<f64, f64>>,
    image: Option<DynamicImage>,
}

struct AppliedState {}

#[derive(Default)]
enum AppStates {
    #[default]
    GenerateOrAnalyze,
    Analyze(AnalyzeStates),
    Generate,
    Apply(ApplyStates),
}

#[derive(Default)]
struct CurvedApp {
    state: AppStates,
    debug: bool,
    image_state: Option<ImageState>,
    preview: Option<TextureBufferedImage>,
    process: String,
    notes: String,
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

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Curved",
        native_options,
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
                    if ui.button("Generate Step Wedge").clicked() {
                        self.state = AppStates::Generate;
                    }
                    if ui.button("Analyze Scan").clicked() {
                        self.state = AppStates::Analyze(AnalyzeStates::WaitingForImage);
                    }
                    if ui.button("Apply Curve").clicked() {
                        self.state = AppStates::Apply(ApplyStates::WaitingForImageAndCurve(
                            WaitingForImageAndCurveState {
                                curve: None,
                                image: None,
                            },
                        ));
                    }
                }
                AppStates::Generate => {
                    // Consider writing these to a label in the image
                    // Maybe store a list of procesess to choose from
                    // and add new ones?
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            let process_label = ui.label("Process: ");
                            ui.text_edit_singleline(&mut self.process)
                                .labelled_by(process_label.id);
                        });
                        ui.horizontal(|ui| {
                            let notes_label = ui.label("Notes: ");
                            ui.text_edit_singleline(&mut self.notes)
                                .labelled_by(notes_label.id);
                        });
                    });

                    if ui.button("Generate").clicked() {
                        if let Some(path) = rfd::FileDialog::new().save_file() {
                            let image = generate::generate(self.debug);
                            image.save(path).unwrap();
                        }
                    }
                }
                AppStates::Apply(apply_state) => match apply_state {
                    ApplyStates::WaitingForImageAndCurve(image_and_curve) => {
                        if ui.button("Select Image").clicked() {
                            if let Some(image_file) = rfd::FileDialog::new().pick_file() {
                                let image = image::open(&image_file).unwrap();
                                let preview = TextureBufferedImage::new(
                                    image_file.clone().into_os_string().into_string().unwrap(),
                                    &image,
                                );
                                image_and_curve.image = Some(image);
                                self.preview = Some(preview)
                            }
                        };
                        if ui.button("select curve").clicked() {
                            if let Some(curve_file) = rfd::FileDialog::new().pick_file() {
                                let curve_data = fs::read_to_string(curve_file).unwrap();
                                let curve =
                                    serde_json::from_str::<Spline<f64, f64>>(&curve_data).unwrap();
                                image_and_curve.curve = Some(curve);
                            }
                        };

                        if let Some(preview) = &mut self.preview {
                            preview.ui(ui);
                        }

                        let can_apply =
                            image_and_curve.curve.is_some() && image_and_curve.image.is_some();

                        ui.add_enabled_ui(can_apply, |ui| {
                            if ui.button("Apply").clicked() {
                                self.state =
                                    AppStates::Apply(ApplyStates::Applied(AppliedState {}));
                            }
                        });
                    }
                    // Maybe don't use a new state here but just shift the waiting state.
                    ApplyStates::Applied(_applied_state) => {
                        if ui.button("Save").clicked() {
                            if let Some(_path) = rfd::FileDialog::new().save_file() {
                                //save resulting image
                            }
                        }
                    }
                },
                AppStates::Analyze(analyze_state) => match analyze_state {
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
                },
            }
        });
    }
}
