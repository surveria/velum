use crate::{
    error::{Error, Result},
    syntax::StaticNameId,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BindingOperand {
    Unresolved,
    Global {
        slot: GlobalSlot,
    },
    EvalVariable {
        slot: GlobalSlot,
    },
    Local {
        scope: ScopeId,
        slot: LocalSlot,
    },
    Upvalue {
        function: FunctionScopeId,
        slot: UpvalueSlot,
    },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct GlobalSlot(u32);

impl GlobalSlot {
    pub fn from_index(index: usize) -> Result<Self> {
        let slot = u32::try_from(index)
            .map_err(|_| Error::limit("global binding slot exceeded supported range"))?;
        Ok(Self(slot))
    }

    pub fn index(self) -> Result<usize> {
        usize::try_from(self.0)
            .map_err(|_| Error::limit("global binding slot exceeded addressable range"))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct LocalSlot(u32);

impl LocalSlot {
    pub fn from_index(index: usize) -> Result<Self> {
        let slot = u32::try_from(index)
            .map_err(|_| Error::limit("local binding slot exceeded supported range"))?;
        Ok(Self(slot))
    }

    pub fn index(self) -> Result<usize> {
        usize::try_from(self.0)
            .map_err(|_| Error::limit("local binding slot exceeded addressable range"))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct UpvalueSlot(u32);

impl UpvalueSlot {
    pub fn from_index(index: usize) -> Result<Self> {
        let slot = u32::try_from(index)
            .map_err(|_| Error::limit("upvalue binding slot exceeded supported range"))?;
        Ok(Self(slot))
    }

    pub fn index(self) -> Result<usize> {
        usize::try_from(self.0)
            .map_err(|_| Error::limit("upvalue binding slot exceeded addressable range"))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct ScopeId(usize);

impl ScopeId {
    pub const fn from_index(index: usize) -> Self {
        Self(index)
    }

    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct FunctionScopeId(usize);

impl FunctionScopeId {
    pub const fn from_index(index: usize) -> Self {
        Self(index)
    }

    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ScopeKind {
    Global,
    EvalVariable,
    Local,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Scope {
    pub parent: Option<ScopeId>,
    pub function: FunctionScopeId,
    pub kind: ScopeKind,
    pub declarations: Vec<Declaration>,
}

impl Scope {
    pub const fn new(parent: Option<ScopeId>, function: FunctionScopeId, kind: ScopeKind) -> Self {
        Self {
            parent,
            function,
            kind,
            declarations: Vec::new(),
        }
    }

    pub fn declaration(&self, name: StaticNameId) -> Option<Declaration> {
        let position = self.declaration_position(name).ok()?;
        self.declarations.get(position).copied()
    }

    pub fn declaration_position(&self, name: StaticNameId) -> std::result::Result<usize, usize> {
        self.declarations
            .binary_search_by(|declaration| declaration.name.cmp(&name))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Declaration {
    pub name: StaticNameId,
    pub scope: ScopeId,
    pub operand: BindingOperand,
}

impl Declaration {
    pub const fn new(name: StaticNameId, scope: ScopeId, operand: BindingOperand) -> Self {
        Self {
            name,
            scope,
            operand,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FunctionScope {
    pub parent: Option<FunctionScopeId>,
    pub upvalues: Vec<DeclarationRef>,
}

impl FunctionScope {
    pub const fn new(parent: Option<FunctionScopeId>) -> Self {
        Self {
            parent,
            upvalues: Vec::new(),
        }
    }

    pub fn upvalue_position(
        &self,
        declaration: DeclarationRef,
    ) -> std::result::Result<usize, usize> {
        if let Some(position) = self
            .upvalues
            .iter()
            .position(|upvalue| *upvalue == declaration)
        {
            return Ok(position);
        }
        Err(self.upvalues.len())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct DeclarationRef {
    pub scope: ScopeId,
    pub name: StaticNameId,
}

impl DeclarationRef {
    pub const fn new(scope: ScopeId, name: StaticNameId) -> Self {
        Self { scope, name }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ScopeContext {
    pub scope: ScopeId,
    pub var_scope: ScopeId,
    pub function: FunctionScopeId,
}

impl ScopeContext {
    pub const fn new(scope: ScopeId, var_scope: ScopeId, function: FunctionScopeId) -> Self {
        Self {
            scope,
            var_scope,
            function,
        }
    }
}
