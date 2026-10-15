//! Paging as the API speaks it: offset pages with their totals, keyset pages
//! after a key, and the direction of an ordering.
//!
//! A request clamps rather than refuses: a size of zero becomes one and a
//! size above the cap becomes the cap, so that a service never builds a
//! page it cannot fetch and a client never gets a `400` for asking too much.

use serde::{Deserialize, Serialize};
use turso_orm::Order;

/// The page size when the request names none.
const DEFAULT_SIZE: u64 = 20;
/// The largest page a request can ask for.
const MAX_SIZE: u64 = 100;

/// The default page size, for `serde(default)`.
const fn default_size() -> u64 {
    DEFAULT_SIZE
}

// --8<-- [start:page_request]
/// An offset page, counted from zero, with a size already within bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    /// The page to return.
    page: u64,
    /// Items per page, between one and the cap.
    size: u64,
}

impl PageRequest {
    /// Builds a request from what a client sent, clamping the size into
    /// bounds and defaulting what is missing.
    pub fn new(page: Option<u64>, size: Option<u64>) -> Self {
        Self {
            page: page.unwrap_or(0),
            size: size.unwrap_or(DEFAULT_SIZE).clamp(1, MAX_SIZE),
        }
    }

    /// The page to return.
    pub fn page(self) -> u64 {
        self.page
    }

    /// Items per page.
    pub fn size(self) -> u64 {
        self.size
    }
}

impl Default for PageRequest {
    fn default() -> Self {
        Self::new(None, None)
    }
}

/// A page of items with the totals a listing shows.
#[derive(Debug, Serialize)]
pub struct Page<T> {
    /// The items of the page.
    pub items: Vec<T>,
    /// The page returned.
    pub page: u64,
    /// The number of pages.
    pub pages: u64,
    /// The number of items over every page.
    pub total: u64,
}

impl<T> Page<T> {
    /// Assembles a page from the items fetched and the totals counted.
    pub fn new(items: Vec<T>, request: PageRequest, total: u64, pages: u64) -> Self {
        Self {
            items,
            page: request.page(),
            pages,
            total,
        }
    }

    /// Converts the items and keeps the totals.
    pub fn map<U>(self, f: impl FnMut(T) -> U) -> Page<U> {
        Page {
            items: self.items.into_iter().map(f).collect(),
            page: self.page,
            pages: self.pages,
            total: self.total,
        }
    }
}
// --8<-- [end:page_request]

/// `?after=&size=`: keyset paging, the rows after a key.
#[derive(Debug, Deserialize)]
pub struct After {
    /// The key of the last row seen, absent for the first page.
    pub after: Option<i32>,
    /// Items per page, clamped by [`After::size`].
    #[serde(default = "default_size")]
    size: u64,
}

impl After {
    /// Items per page, between one and the cap.
    pub fn size(&self) -> u64 {
        self.size.clamp(1, MAX_SIZE)
    }
}

/// `?direction=asc|desc`: the direction of an ordering.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Smallest first.
    #[default]
    Asc,
    /// Largest first.
    Desc,
}

impl From<Direction> for Order {
    fn from(direction: Direction) -> Self {
        match direction {
            Direction::Asc => Order::Asc,
            Direction::Desc => Order::Desc,
        }
    }
}
