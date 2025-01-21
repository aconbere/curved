use std::fs;
use std::path::PathBuf;

use eframe::egui;
use image;
use image::DynamicImage;
use splines::Spline;

use super::analyze;
use super::apply;
use super::generate;

mod texture_buffered_image;

use texture_buffered_image::TextureBufferedImage;

struct PreviewedImage {
    path: PathBuf,
    image: DynamicImage,
    preview: TextureBufferedImage,
}

#[derive(Default)]
struct ApplyPageState {
    curve: Option<Spline<f64, f64>>,
    image: Option<PreviewedImage>,
    curved_image: Option<PreviewedImage>,
}

#[derive(Default)]
struct GeneratePageState {
    process: String,
    notes: String,
    image: Option<PreviewedImage>,
}

#[derive(Default, PartialEq)]
enum AnalyzePreviewTab {
    #[default]
    Scan,
    Results,
}

#[derive(Default)]
struct AnalyzePageState {
    scan: Option<PreviewedImage>,
    analysis: Option<analyze::AnalyzeResults>,
    preview_tab: AnalyzePreviewTab,
}

#[derive(Default, PartialEq)]
enum Page {
    #[default]
    Home,
    Analyze,
    Generate,
    Apply,
}

#[derive(Default)]
struct CurvedApp {
    debug: bool,

    page: Page,
    generate_page_state: GeneratePageState,
    analyze_page_state: AnalyzePageState,
    apply_page_state: ApplyPageState,
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
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 900.0]),
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

fn preview_area(ui: &mut egui::Ui, texture: &mut TextureBufferedImage) {
    egui::Frame::none().fill(egui::Color32::RED).show(ui, |ui| {
        ui.set_min_width(500.0);
        ui.set_min_height(500.0);
        ui.vertical(|ui| {
            texture.ui(ui);
        });
    });
}

fn tab_bar(ui: &mut egui::Ui, app: &mut CurvedApp) {
    let buttons = vec![
        (Page::Home, egui::Button::new("home")),
        (Page::Generate, egui::Button::new("generate")),
        (Page::Analyze, egui::Button::new("analyze")),
        (Page::Apply, egui::Button::new("apply")),
    ];

    ui.horizontal(|ui| {
        for (name, button) in buttons {
            let enabled = name != app.page;
            if ui.add_enabled(enabled, button).clicked() {
                app.page = name;
            };
        }
    });
}

fn home_page(ui: &mut egui::Ui) {
    ui.heading("Curved");
    ui.separator();
    ui.add_space(12.0);
    ui.label(
        "Curved is a tool to help you build tone adjustment curves for analog printing. To use it \
         first generate a step wedge. Then go print the step wedge however you like. Scan the \
         results and use the Analyze function to produce a tone adjustment curve. Finally you can \
         apply the curve to an image with the Apply tool.",
    );
}

fn generate_page(ui: &mut egui::Ui, state: &mut GeneratePageState, debug: bool) {
    let mut process = state.process.clone();
    let mut notes = state.notes.clone();

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_max_width(300.0);
            ui.label(
                "Generates a step wedge for printing as a transparency. Output will be a 16bit \
                 greyscale image. It is generated in its \"inverted\" form and does not need to \
                 be further inverted before printing. A 300 dpi print should be about 5\"x5.25\".",
            );

            let process_label = ui.label("Process: ");
            ui.text_edit_singleline(&mut process)
                .labelled_by(process_label.id);

            let notes_label = ui.label("Notes: ");
            ui.text_edit_singleline(&mut notes)
                .labelled_by(notes_label.id);

            if ui.button("Generate").clicked() {
                let image = generate::generate(debug);
                let preview = TextureBufferedImage::new(
                    format!("generated_step_wedge_{}_{}", state.process, state.notes),
                    &image,
                );
                state.image = Some(PreviewedImage {
                    path: PathBuf::new(),
                    image,
                    preview,
                });
            }
            if let Some(image) = &state.image {
                if ui.button("Save").clicked() {
                    if let Some(path) = rfd::FileDialog::new().save_file() {
                        image.image.save(path).unwrap();
                    }
                };
            } else {
                ui.add_enabled(false, egui::Button::new("Save"));
            }
        });

        if let Some(image) = &mut state.image {
            preview_area(ui, &mut image.preview)
        }

        state.process = process.to_string();
        state.notes = notes.to_string();
    });
}

fn apply_page(ui: &mut egui::Ui, state: &mut ApplyPageState) {
    if ui.button("Select Image").clicked() {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            let image = image::open(&path).unwrap();
            let preview = TextureBufferedImage::new(
                path.clone().into_os_string().into_string().unwrap(),
                &image,
            );
            state.image = Some(PreviewedImage {
                path,
                image,
                preview,
            });
        }
    };

    if ui.button("select curve").clicked() {
        if let Some(curve_file) = rfd::FileDialog::new().pick_file() {
            let curve_data = fs::read_to_string(curve_file).unwrap();
            let curve = serde_json::from_str::<Spline<f64, f64>>(&curve_data).unwrap();
            state.curve = Some(curve);
        }
    };

    if let Some(image) = &mut state.image {
        image.preview.ui(ui);
    }

    if let (Some(image), Some(curve)) = (&state.image, &state.curve) {
        if ui.button("Apply").clicked() {
            let curved_image = apply::apply(&image.image, &curve);

            let preview =
                TextureBufferedImage::new("curved_image_preview".to_string(), &curved_image);
            state.curved_image = Some(PreviewedImage {
                path: image.path.clone(),
                image: curved_image,
                preview,
            });
        }
    }

    if let Some(curved_image) = &state.curved_image {
        if ui.button("Save").clicked() {
            if let Some(path) = rfd::FileDialog::new().save_file() {
                curved_image.image.save(path).unwrap();
            }
        }
    }
}

fn analyze_results(ui: &mut egui::Ui, state: &mut AnalyzePageState) {
    let buttons = vec![
        (AnalyzePreviewTab::Scan, egui::Button::new("Scan")),
        (AnalyzePreviewTab::Results, egui::Button::new("Results")),
    ];

    egui::Frame::none()
        .fill(egui::Color32::BLUE)
        .show(ui, |ui| {
            ui.set_min_width(700.0);
            ui.set_min_height(700.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    for (name, button) in buttons {
                        let enabled = name != state.preview_tab;
                        if ui.add_enabled(enabled, button).clicked() {
                            state.preview_tab = name;
                        };
                    }
                });
            });
            if let Some(scan) = &mut state.scan {
                preview_area(ui, &mut scan.preview);

                let path: String = scan.path.to_string_lossy().to_string();
                ui.monospace(path);
            }
            if let Some(analysis) = &state.analysis {
                if ui.button("save").clicked() {
                    if let Some(path) = rfd::FileDialog::new().save_file() {
                        let curve_file = fs::File::create(path).unwrap();
                        serde_json::to_writer(&curve_file, &analysis.curve).unwrap();
                    }
                };
            } else {
                let _ = ui.button("save");
            }
        });
}

fn analyze_page(ui: &mut egui::Ui, state: &mut AnalyzePageState, debug: bool) {
    ui.label(
        "This tool will evaluate a scan of the generated step wedge to generate a new curve file.",
    );

    // Draw the side bar
    //

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(300.0);
            if ui.button("Select Scan").clicked() {
                if let Some(file) = rfd::FileDialog::new().pick_file() {
                    let path = PathBuf::from(file.display().to_string());
                    let image = image::open(&path).unwrap();

                    let preview = TextureBufferedImage::new(
                        path.clone().into_os_string().into_string().unwrap(),
                        &image,
                    );

                    state.scan = Some(PreviewedImage {
                        path: path.clone(),
                        image,
                        preview,
                    });
                }
            }

            if let Some(scan) = &state.scan {
                if ui.add_enabled(true, egui::Button::new("analyze")).clicked() {
                    let analyze_results = analyze::analyze(&scan.image, debug).unwrap();
                    state.analysis = Some(analyze_results);
                }
            } else {
                ui.add_enabled(false, egui::Button::new("analyze"));
            }
        });

        ui.vertical(|ui| {
            analyze_results(ui, state);
        });
    });
}

impl eframe::App for CurvedApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            tab_bar(ui, self);
            ui.separator();
            ui.add_space(12.0);
            match &mut self.page {
                Page::Home => {
                    home_page(ui);
                }
                Page::Generate => {
                    generate_page(ui, &mut self.generate_page_state, self.debug);
                }
                Page::Apply => {
                    apply_page(ui, &mut self.apply_page_state);
                }
                Page::Analyze => {
                    analyze_page(ui, &mut self.analyze_page_state, self.debug);
                }
            }
        });
    }
}
