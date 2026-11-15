//! Local music library: folder scan, tags, search, a catalog view and the library database
//! ([`store`], `library.db`) and its local playlists ([`playlists`]).

pub mod catalog;
pub mod import;
pub mod library;
pub mod playlists;
pub mod scan;
pub mod store;
pub mod tags;

pub use catalog::LocalCatalog;
pub use library::LocalLibrary;
