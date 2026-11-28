#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod cloud;
mod desktop;
mod service;
mod settings;
mod single_instance;
mod updates;
mod workspace;

fn main() {
    desktop::run();
}
