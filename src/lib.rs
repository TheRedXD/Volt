pub mod blerp;
// TODO: Move everything into components (visual)
pub mod browser;
mod error;
pub mod images;
pub mod info;
mod test;
pub mod visual;

pub type Result<T> = std::result::Result<T, error::VoltError>;
