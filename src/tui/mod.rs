pub mod app;
pub mod highlight;
pub mod input;
pub mod output;
pub mod theme;
pub mod ui;

#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use app::{App, Status};
#[allow(unused_imports)]
pub use input::InputState;
#[allow(unused_imports)]
pub use output::{OutputBuffer, OutputLine};
#[allow(unused_imports)]
pub use theme::Theme;
