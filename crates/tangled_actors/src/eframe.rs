use std::{
    sync::Arc,
    task::{Context, Wake, Waker},
};

use eframe::egui;
use tokio::sync::mpsc;

use crate::{Actor, ActorCtx, ActorLink, ActorSync, WeakLink};

struct EguiWaker(egui::Context);

impl EguiWaker {
    fn new(ctx: &egui::Context) -> Arc<Self> {
        Arc::new(Self(ctx.clone()))
    }
}

impl Wake for EguiWaker {
    fn wake(self: std::sync::Arc<Self>) {
        self.0.request_repaint();
    }
}

/// Wrapper around [`eframe::App`] that also runs actor's main loop.
///
/// Note: this actor can only have sync message handlers.
pub struct ActorApp<A: Actor> {
    inner: A,
    receiver: mpsc::UnboundedReceiver<A::Message>,
    waker: Waker,
    // Store a link to ensure a channel is alive for as long as the app lives.
    _link: ActorLink<A>,
}

impl<A: ActorSync + eframe::App> ActorApp<A> {
    pub fn new(cc: &eframe::CreationContext, inner_builder: impl FnOnce(ActorCtx<A>) -> A) -> Self {
        let waker = Waker::from(EguiWaker::new(&cc.egui_ctx));

        let (sender, receiver) = mpsc::unbounded_channel();
        let link = ActorLink { sender };
        let weak_link = WeakLink::from(&link);
        let ctx = ActorCtx { weak_link };
        let inner = inner_builder(ctx);
        Self {
            inner,
            receiver,
            waker,
            _link: link,
        }
    }
}

impl<T: ActorSync + eframe::App> eframe::App for ActorApp<T> {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, frame: &mut eframe::Frame) {
        self.inner.ui(ui, frame);
    }

    fn logic(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        loop {
            match self
                .receiver
                .poll_recv(&mut Context::from_waker(&self.waker))
            {
                std::task::Poll::Ready(Some(message)) => {
                    self.inner.process_message_sync(message);
                }
                std::task::Poll::Ready(None) | std::task::Poll::Pending => break,
            }
        }
        self.inner.logic(ctx, frame);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.inner.save(storage);
    }

    fn on_exit(&mut self) {
        self.inner.on_exit();
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        self.inner.auto_save_interval()
    }

    fn clear_color(&self, visuals: &eframe::egui::Visuals) -> [f32; 4] {
        self.inner.clear_color(visuals)
    }

    fn persist_egui_memory(&self) -> bool {
        self.inner.persist_egui_memory()
    }

    fn raw_input_hook(
        &mut self,
        ctx: &eframe::egui::Context,
        raw_input: &mut eframe::egui::RawInput,
    ) {
        self.inner.raw_input_hook(ctx, raw_input);
    }
}
