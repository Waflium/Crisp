use crate::util::uri::URI;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KnownLinkType {
    BugReport,
}

impl KnownLinkType {
    pub fn create(self, link: URI) -> Entry {
        Entry { ty: self, link }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    ty: KnownLinkType,
    link: URI,
}

pub fn write_untrusted(entries: &[Entry], p: &mut crisp_protocol::Writer) {
    p.varint(entries.len() as i32);
    for entry in entries {
        p.bool(true);
        p.varint(entry.ty as i32);
        p.string(&entry.link.to_string());
    }
}
