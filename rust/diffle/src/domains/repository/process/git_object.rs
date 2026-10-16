//! What `git cat-file` says about one object: its kind, its size, and the first bytes of its body.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Blob,
    Tree,
    Commit,
    Tag,
}

impl ObjectKind {
    pub(super) fn parse(name: &str) -> Option<Self> {
        match name {
            "blob" => Some(Self::Blob),
            "tree" => Some(Self::Tree),
            "commit" => Some(Self::Commit),
            "tag" => Some(Self::Tag),
            _ => None,
        }
    }
}

/// An object's kind and size, and the first bytes of its body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitObject {
    kind: ObjectKind,
    size: u64,
    head: Vec<u8>,
}

impl GitObject {
    pub(super) const fn new(kind: ObjectKind, size: u64, head: Vec<u8>) -> Self {
        Self { kind, size, head }
    }

    #[must_use]
    pub const fn kind(&self) -> ObjectKind {
        self.kind
    }

    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    /// As many bytes as were asked for, or all of them when the object is smaller.
    #[must_use]
    pub fn head(&self) -> &[u8] {
        &self.head
    }

    #[must_use]
    pub fn into_head(self) -> Vec<u8> {
        self.head
    }
}
