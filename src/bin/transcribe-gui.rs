use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Duration;
use transcribe::runner::{self, Config, Event};

const LANGUAGES: &[(&str, &str)] = &[
    ("auto", "Auto detecteer"),
    ("nl", "Nederlands"),
    ("en", "English"),
    ("de", "Deutsch"),
    ("fr", "Français"),
    ("es", "Español"),
    ("it", "Italiano"),
    ("pt", "Português"),
];

const MODELS: &[(&str, &str)] = &[
    ("tiny", "tiny (snel, lage kwaliteit, ~75MB)"),
    ("base", "base (~140MB)"),
    ("small", "small (~470MB)"),
    ("medium", "medium (~1.5GB)"),
    ("large-v3", "large-v3 (beste kwaliteit, ~3GB)"),
];

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([640.0, 540.0])
            .with_min_inner_size([480.0, 420.0])
            .with_title("Transcribe"),
        ..Default::default()
    };
    eframe::run_native(
        "Transcribe",
        options,
        Box::new(|_cc| Ok(Box::new(App::default()))),
    )
}

struct App {
    path: Option<PathBuf>,
    language: String,
    model: String,
    force: bool,
    running: bool,
    log: Vec<String>,
    rx: Option<Receiver<UiMsg>>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            path: None,
            language: "auto".into(),
            model: "large-v3".into(),
            force: false,
            running: false,
            log: Vec::new(),
            rx: None,
        }
    }
}

enum UiMsg {
    Line(String),
    Finished,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_messages(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(4.0);
            ui.heading("Transcribe");
            ui.label("MP3/MP4 → tekst via whisper.cpp");
            ui.add_space(12.0);

            // Source picker
            ui.group(|ui| {
                ui.label(egui::RichText::new("Bron").strong());
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!self.running, egui::Button::new("📁  Kies map…"))
                        .clicked()
                    {
                        if let Some(p) = rfd::FileDialog::new().pick_folder() {
                            self.path = Some(p);
                        }
                    }
                    if ui
                        .add_enabled(!self.running, egui::Button::new("🎬  Kies file…"))
                        .clicked()
                    {
                        if let Some(p) = rfd::FileDialog::new()
                            .add_filter("Audio/Video", &["mp3", "mp4"])
                            .pick_file()
                        {
                            self.path = Some(p);
                        }
                    }
                });
                match &self.path {
                    Some(p) => ui.label(format!("→ {}", p.display())),
                    None => ui.label(egui::RichText::new("(nog niets gekozen)").italics()),
                };
            });

            ui.add_space(8.0);

            // Options
            ui.group(|ui| {
                ui.label(egui::RichText::new("Opties").strong());

                ui.horizontal(|ui| {
                    ui.label("Taal:");
                    let current = LANGUAGES
                        .iter()
                        .find(|(c, _)| *c == self.language)
                        .map(|(_, l)| *l)
                        .unwrap_or(self.language.as_str());
                    egui::ComboBox::from_id_salt("lang")
                        .selected_text(current)
                        .show_ui(ui, |ui| {
                            for (code, label) in LANGUAGES {
                                ui.selectable_value(&mut self.language, code.to_string(), *label);
                            }
                        });
                });

                ui.horizontal(|ui| {
                    ui.label("Model:");
                    let current = MODELS
                        .iter()
                        .find(|(c, _)| *c == self.model)
                        .map(|(_, l)| *l)
                        .unwrap_or(self.model.as_str());
                    egui::ComboBox::from_id_salt("model")
                        .selected_text(current)
                        .show_ui(ui, |ui| {
                            for (code, label) in MODELS {
                                ui.selectable_value(&mut self.model, code.to_string(), *label);
                            }
                        });
                });

                ui.checkbox(
                    &mut self.force,
                    "Her-transcribeer ook als .txt al bestaat",
                );
            });

            ui.add_space(12.0);

            // Start
            let can_start = self.path.is_some() && !self.running;
            let label = if self.running {
                "⏳  Bezig…"
            } else {
                "▶  Start"
            };
            let button = egui::Button::new(egui::RichText::new(label).size(16.0))
                .min_size(egui::vec2(140.0, 36.0));
            if ui.add_enabled(can_start, button).clicked() {
                self.start();
            }

            ui.add_space(4.0);
            ui.separator();

            // Log
            ui.label(egui::RichText::new("Log").strong());
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if self.log.is_empty() {
                        ui.label(egui::RichText::new("(nog niets gedraaid)").italics());
                    } else {
                        for line in &self.log {
                            ui.monospace(line);
                        }
                    }
                });
        });
    }
}

impl App {
    fn drain_messages(&mut self, ctx: &egui::Context) {
        let mut finished = false;
        if let Some(rx) = &self.rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    UiMsg::Line(s) => self.log.push(s),
                    UiMsg::Finished => finished = true,
                }
            }
            ctx.request_repaint_after(Duration::from_millis(200));
        }
        if finished {
            self.running = false;
            self.rx = None;
        }
    }

    fn start(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let config = Config {
            path,
            language: self.language.clone(),
            model: self.model.clone(),
            force: self.force,
        };

        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.running = true;
        self.log.clear();
        self.log.push("Gestart…".to_string());

        thread::spawn(move || {
            let result = runner::run(config, |event| {
                let _ = tx.send(UiMsg::Line(format_event(&event)));
            });
            if let Err(e) = result {
                let _ = tx.send(UiMsg::Line(format!("Fout: {e:#}")));
            }
            let _ = tx.send(UiMsg::Finished);
        });
    }
}

fn format_event(event: &Event) -> String {
    match event {
        Event::NoFilesFound { path } => {
            format!("Geen .mp3 of .mp4 in {}", path.display())
        }
        Event::FilesFound { count } => format!("{count} file(s) gevonden"),
        Event::ModelLoading { path } => format!("Model laden: {}", path.display()),
        Event::FileStart {
            index,
            total,
            path,
        } => format!("[{}/{}] {}", index + 1, total, path.display()),
        Event::FileSkipped {
            index,
            total,
            path,
        } => format!(
            "[{}/{}] overgeslagen (txt bestaat): {}",
            index + 1,
            total,
            path.display()
        ),
        Event::FileDecoded {
            samples,
            duration_s,
            elapsed_s,
        } => format!(
            "  audio: {} samples ({:.1}s, decode {:.1}s)",
            samples, duration_s, elapsed_s
        ),
        Event::FileTranscribed {
            txt_path,
            chars,
            elapsed_s,
        } => format!(
            "  ✓ {} ({} tekens, {:.1}s)",
            txt_path.display(),
            chars,
            elapsed_s
        ),
        Event::FileError { path, error } => {
            format!("  ! {}: {}", path.display(), error)
        }
        Event::Done => "Klaar.".to_string(),
    }
}
