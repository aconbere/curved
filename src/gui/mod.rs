use std::fs;
use std::path::PathBuf;

use anyhow;
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
    analysis_preview: Option<TextureBufferedImage>,
    preview_tab: AnalyzePreviewTab,
}

#[derive(Default, PartialEq)]
enum Page {
    #[default]
    Generate,
    Analyze,
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

fn draw_analyze_preview(
    curve: &Spline<f64, f64>,
    histogram: &Vec<u32>,
) -> anyhow::Result<TextureBufferedImage> {
    let mut image: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        image::ImageBuffer::new(1024, 1024);
    analyze::draw_curve(&mut image, &curve)?;
    analyze::draw_histogram(&mut image, &histogram);
    Ok(TextureBufferedImage::new(
        format!("curve_and_histogram"),
        &DynamicImage::ImageRgb8(image),
    ))
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

fn tab_bar(ui: &mut egui::Ui, app: &mut CurvedApp) {
    egui::TopBottomPanel::top("top_tab_bar")
        .resizable(false)
        .min_height(32.0)
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Curved");
                ui.separator();
                ui.selectable_value(&mut app.page, Page::Generate, "Generate");
                ui.selectable_value(&mut app.page, Page::Analyze, "Analyze");
                ui.selectable_value(&mut app.page, Page::Apply, "Apply");
            });
        });
}

fn generate_page(ui: &mut egui::Ui, state: &mut GeneratePageState, debug: bool) {
    let mut process = state.process.clone();
    let mut notes = state.notes.clone();

    egui::SidePanel::left("side_bar")
        .default_width(325.0)
        .show_inside(ui, |ui| {
            ui.label(
                "Generates a step wedge for printing as a transparency. Output will be a 16bit \
                 greyscale image. It is generated in its \"inverted\" form and does not need to \
                 be further inverted before printing. A 300 dpi print should be about 5\"x5.25\".",
            );

            ui.separator();

            let process_label = ui.label("Process: ");
            ui.text_edit_singleline(&mut process)
                .labelled_by(process_label.id);

            let notes_label = ui.label("Notes: ");
            ui.text_edit_singleline(&mut notes)
                .labelled_by(notes_label.id);

            if ui.button("Generate").clicked() {
                let image = generate::generate(debug).unwrap();
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
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        egui::TopBottomPanel::bottom("image_commands")
            .min_height(32.0)
            .show_inside(ui, |ui| {
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
        egui::CentralPanel::default().show_inside(ui, |ui| {
            if let Some(image) = &mut state.image {
                image.preview.ui(ui);
            }
        });
    });

    state.process = process.to_string();
    state.notes = notes.to_string();
}

fn apply_page(ui: &mut egui::Ui, state: &mut ApplyPageState) {
    egui::SidePanel::left("side_bar")
        .min_width(325.0)
        .show_inside(ui, |ui| {
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
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        egui::TopBottomPanel::bottom("controls")
            .min_height(32.0)
            .show_inside(ui, |ui| {
                if state.curved_image.is_some() {
                    ui.horizontal(|ui| {
                        if let Some(ci) = &state.curved_image {
                            if ui.button("Save").clicked() {
                                if let Some(path) = rfd::FileDialog::new().save_file() {
                                    ci.image.save(path).unwrap();
                                }
                            }
                            if ui.button("Undo").clicked() {
                                state.curved_image = None;
                            }
                        }
                    });
                } else if let Some(image) = &state.image {
                    if ui.button("Apply Curve").clicked() {
                        if let Some(curve_file) = rfd::FileDialog::new().pick_file() {
                            let curve_data = fs::read_to_string(curve_file).unwrap();
                            let curve =
                                serde_json::from_str::<Spline<f64, f64>>(&curve_data).unwrap();
                            let curved_image = apply::apply(&image.image, &curve);
                            state.curve = Some(curve);

                            let preview = TextureBufferedImage::new(
                                "curved_image_preview".to_string(),
                                &curved_image,
                            );
                            state.curved_image = Some(PreviewedImage {
                                path: image.path.clone(),
                                image: curved_image,
                                preview,
                            });
                        }
                    };
                }
            });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            if let Some(ci) = &mut state.curved_image {
                ci.preview.ui(ui);
            } else if let Some(image) = &mut state.image {
                image.preview.ui(ui);
            }
        });
    });
}

fn analyze_page(ui: &mut egui::Ui, state: &mut AnalyzePageState, debug: bool) {
    egui::SidePanel::left("side_bar")
        .min_width(325.0)
        .show_inside(ui, |ui| {
            ui.label(
                "Analyze evaluates a scan of the generated step wedge to generate a new curve \
                 file.",
            );
            ui.separator();
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
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        egui::TopBottomPanel::top("analyze_tabs")
            .min_height(32.0)
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut state.preview_tab, AnalyzePreviewTab::Scan, "Scan");
                    ui.selectable_value(
                        &mut state.preview_tab,
                        AnalyzePreviewTab::Results,
                        "Results",
                    );
                });
            });
        egui::TopBottomPanel::bottom("actions")
            .min_height(32.0)
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| match state.preview_tab {
                    AnalyzePreviewTab::Scan => {
                        if let Some(scan) = &state.scan {
                            if ui.add_enabled(true, egui::Button::new("analyze")).clicked() {
                                let analyze_results = analyze::analyze(&scan.image, debug).unwrap();
                                state.analysis_preview = Some(
                                    draw_analyze_preview(
                                        &analyze_results.curve,
                                        &analyze_results.histogram,
                                    )
                                    .unwrap(),
                                );
                                state.analysis = Some(analyze_results);
                                state.preview_tab = AnalyzePreviewTab::Results;
                            }
                        } else {
                            ui.add_enabled(false, egui::Button::new("analyze"));
                        }
                        if let Some(scan) = &mut state.scan {
                            let path: String = scan.path.to_string_lossy().to_string();
                            ui.monospace(path);
                        }
                    }
                    AnalyzePreviewTab::Results => {
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
                    }
                });
            });
        egui::CentralPanel::default().show_inside(ui, |ui| match state.preview_tab {
            AnalyzePreviewTab::Scan => {
                if let Some(scan) = &mut state.scan {
                    scan.preview.ui(ui);
                }
            }
            AnalyzePreviewTab::Results => {
                if let Some(preview) = &mut state.analysis_preview {
                    preview.ui(ui);
                }
            }
        });
    });
}

impl eframe::App for CurvedApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            tab_bar(ui, self);
            match &mut self.page {
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
