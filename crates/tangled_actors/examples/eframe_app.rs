use std::{thread, time::Duration};

use eframe::{App, CreationContext, NativeOptions, egui::CentralPanel};
use tangled_actors::{ActorCtx, eframe::ActorApp, make_actor};

struct MyApp {
    counter: usize,
    ctx: ActorCtx<Self>,
}

impl MyApp {
    fn new(ctx: ActorCtx<Self>, _cc: &CreationContext) -> Self {
        Self { counter: 0, ctx }
    }
}

#[make_actor]
impl MyApp {
    fn increment(&mut self) {
        self.counter += 1;
    }
}

impl App for MyApp {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        CentralPanel::default().show(ui, |ui| {
            ui.label(format!("Counter: {}", self.counter));
            if ui.button("Increment").clicked() {
                // Send a message to itself to increment the counter.
                self.ctx.link().increment().away().unwrap();
            }
            if ui.button("Increment 2").clicked() {
                // Original function still exists in case you wish to call it.
                self.increment();
            }
            if ui.button("Increment in a thread").clicked() {
                // Spawn a thread to increment a counter multiple times.
                let link = self.ctx.link();
                thread::spawn(move || {
                    for _ in 0..10 {
                        thread::sleep(Duration::from_secs(1));
                        link.increment().away().unwrap();
                    }
                });
            }
        });
    }
}

fn main() {
    eframe::run_native(
        "tangled actors test app",
        NativeOptions::default(),
        Box::new(|cc| Ok(Box::new(ActorApp::new(cc, |ctx| MyApp::new(ctx, cc))))),
    )
    .unwrap();
}
