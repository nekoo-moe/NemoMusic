//! Local music library: folder scan, tags, search, a catalog view and the library database
//! ([`store`], `library.db`).

pub mod catalog;
pub mod library;
pub mod scan;
pub mod store;
pub mod tags;

pub use catalog::LocalCatalog;
pub use library::LocalLibrary;
