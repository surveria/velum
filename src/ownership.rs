use std::{
    fmt,
    hash::{Hash, Hasher},
    rc::Rc,
};

const INITIAL_VM_GENERATION: u64 = 1;

#[derive(Debug)]
struct VmOwnerToken {
    generation: VmGeneration,
}

/// Opaque identity of one VM-owned storage generation.
///
/// Identity is capability-based instead of process-global and numeric. Clones
/// refer to the same VM owner, while independently created identities cannot
/// alias. The owner token remains allocated as long as a local value retains
/// an identity, so a later VM cannot reuse it accidentally.
#[derive(Clone)]
pub struct VmIdentity {
    owner: Rc<VmOwnerToken>,
}

impl VmIdentity {
    pub(crate) fn new() -> Self {
        Self {
            owner: Rc::new(VmOwnerToken {
                generation: VmGeneration::initial(),
            }),
        }
    }

    /// Returns the VM storage generation represented by this identity.
    #[must_use]
    pub fn generation(&self) -> VmGeneration {
        self.owner.generation
    }
}

impl fmt::Debug for VmIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VmIdentity")
            .field("generation", &self.generation())
            .finish_non_exhaustive()
    }
}

impl PartialEq for VmIdentity {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.owner, &other.owner)
    }
}

impl Eq for VmIdentity {}

impl Hash for VmIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Rc::as_ptr(&self.owner).hash(state);
    }
}

/// Generation of the VM-owned stores covered by a [`VmIdentity`].
///
/// The initial implementation creates one non-reused generation per VM. The
/// explicit type prevents future arena reuse or reset support from silently
/// treating stale handles as current.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VmGeneration(u64);

impl VmGeneration {
    const fn initial() -> Self {
        Self(INITIAL_VM_GENERATION)
    }
}
