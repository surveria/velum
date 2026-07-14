use crate::SourceSpan;

/// One frontend node with its complete source range.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AstNode<K> {
    kind: K,
    span: SourceSpan,
}

impl<K> AstNode<K> {
    pub const fn new(kind: K, span: SourceSpan) -> Self {
        Self { kind, span }
    }

    pub const fn kind(&self) -> &K {
        &self.kind
    }

    pub(crate) const fn kind_mut(&mut self) -> &mut K {
        &mut self.kind
    }

    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    pub fn into_kind(self) -> K {
        self.kind
    }
}
