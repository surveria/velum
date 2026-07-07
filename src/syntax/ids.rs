use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct StaticNameId(u32);

impl StaticNameId {
    pub fn from_index(index: usize) -> Result<Self> {
        let id = u32::try_from(index)
            .map_err(|_| Error::limit("static name table exceeded supported range"))?;
        Ok(Self(id))
    }

    pub fn index(self) -> Result<usize> {
        usize::try_from(self.0).map_err(|_| Error::limit("static name id exceeded supported range"))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct StaticStringId(u32);

impl StaticStringId {
    pub fn from_index(index: usize) -> Result<Self> {
        let id = u32::try_from(index)
            .map_err(|_| Error::limit("static string table exceeded supported range"))?;
        Ok(Self(id))
    }

    pub fn index(self) -> Result<usize> {
        usize::try_from(self.0)
            .map_err(|_| Error::limit("static string id exceeded supported range"))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct StaticBindingId(u32);

impl StaticBindingId {
    pub fn from_index(index: usize) -> Result<Self> {
        let id = u32::try_from(index)
            .map_err(|_| Error::limit("static binding table exceeded supported range"))?;
        Ok(Self(id))
    }

    pub fn index(self) -> Result<usize> {
        usize::try_from(self.0)
            .map_err(|_| Error::limit("static binding id exceeded supported range"))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct StaticFunctionId(u32);

impl StaticFunctionId {
    pub fn from_index(index: usize) -> Result<Self> {
        let id = u32::try_from(index)
            .map_err(|_| Error::limit("static function table exceeded supported range"))?;
        Ok(Self(id))
    }

    pub fn index(self) -> Result<usize> {
        usize::try_from(self.0)
            .map_err(|_| Error::limit("static function id exceeded addressable range"))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct StaticPropertyAccessId(u32);

impl StaticPropertyAccessId {
    pub fn from_index(index: usize) -> Result<Self> {
        let id = u32::try_from(index)
            .map_err(|_| Error::limit("static property access table exceeded supported range"))?;
        Ok(Self(id))
    }

    pub fn index(self) -> Result<usize> {
        usize::try_from(self.0)
            .map_err(|_| Error::limit("static property access id exceeded supported range"))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct StaticCallSiteId(u32);

impl StaticCallSiteId {
    pub fn from_index(index: usize) -> Result<Self> {
        let id = u32::try_from(index)
            .map_err(|_| Error::limit("static call site table exceeded supported range"))?;
        Ok(Self(id))
    }

    pub fn index(self) -> Result<usize> {
        usize::try_from(self.0)
            .map_err(|_| Error::limit("static call site id exceeded supported range"))
    }
}
