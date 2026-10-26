//! The main window.

use eframe::egui;

use crate::editor::Editor;
use crate::theme::{self, ThemeId};
use crate::views;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Editor,
    Library,
}

pub struct KatanaApp {
    pub editor: Editor,
    pub tab: Tab,
    pub theme: ThemeId,
    /// `layout::BlockDef::id` of the block shown in the detail panel.
    pub selected_block: &'static str,
    /// Last failed action, shown in the status line.
    pub error: Option<String>,
    applied_theme: Option<ThemeId>,
}

impl Default for KatanaApp {
    fn default() -> Self {
        Self::new()
    }
}

impl KatanaApp {
    pub fn new() -> KatanaApp {
        KatanaApp {
            editor: Editor::new(),
            tab: Tab::Editor,
            theme: ThemeId::Dark,
            selected_block: "booster",
            error: None,
            applied_theme: None,
        }
    }

    /// The whole UI, drawn into the window's root `Ui`.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        if self.applied_theme != Some(self.theme) {
            theme::apply(ui.ctx(), &self.theme.palette());
            self.applied_theme = Some(self.theme);
        }
        let palette = self.theme.palette();
        egui::Panel::top("top_bar").show(ui, |ui| views::top_bar(ui, self, &palette));
        egui::Panel::top("tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Editor, "Editor");
                ui.selectable_value(&mut self.tab, Tab::Library, "Library");
            });
        });
        egui::Panel::bottom("status").show(ui, |ui| views::status(ui, self, &palette));
        egui::CentralPanel::default().show(ui, |ui| match self.tab {
            Tab::Editor => {}
            Tab::Library => views::library(ui),
        });
    }
}

impl eframe::App for KatanaApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}
