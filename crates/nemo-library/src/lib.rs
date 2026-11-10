//! Local music library: folder scan, tags, search and a catalog view.

pub mod catalog;
pub mod library;
pub mod scan;
pub mod tags;

pub use catalog::LocalCatalog;
pub use library::LocalLibrary;
