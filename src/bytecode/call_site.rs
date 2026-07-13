use crate::syntax::StaticCallSiteId;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BytecodeCallSite {
    site: StaticCallSiteId,
}

impl BytecodeCallSite {
    pub(crate) const fn new(site: StaticCallSiteId) -> Self {
        Self { site }
    }

    pub const fn site(self) -> StaticCallSiteId {
        self.site
    }
}
