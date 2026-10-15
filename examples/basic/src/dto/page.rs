//! Paging parameters and the paged response.

use serde::{Deserialize, Serialize};

/// The default number of items per page.
const fn default_size() -> u64 {
    20
}

/// `?page=&size=`: offset paging, pages counted from zero.
#[derive(Debug, Deserialize)]
pub struct Page {
    /// The page to return.
    #[serde(default)]
    pub page: u64,
    /// Items per page.
    #[serde(default = "default_size")]
    pub size: u64,
}

/// `?after=&size=`: keyset paging, the rows after a key.
#[derive(Debug, Deserialize)]
pub struct After {
    /// The key of the last row seen, absent for the first page.
    pub after: Option<i32>,
    /// Items per page.
    #[serde(default = "default_size")]
    pub size: u64,
}

/// A page of items with the totals a listing shows.
#[derive(Debug, Serialize)]
pub struct Paged<T> {
    /// The items of the page.
    pub items: Vec<T>,
    /// The page returned.
    pub page: u64,
    /// The number of pages.
    pub pages: u64,
    /// The number of items over every page.
    pub total: u64,
}
