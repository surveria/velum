use super::BytecodeCompletion;
use super::block::BytecodeBlock;
use super::function::BytecodeFunction;
use super::function_mode::BytecodeNewTargetMode;
use super::numeric::{
    BytecodeNumericBinaryOp, BytecodeNumericCompareOp, BytecodeNumericEqualityOp,
    BytecodeNumericUnaryOp,
};
use super::private::{BytecodeClassMemberKey, BytecodePrivateName};
use super::super_property::BytecodeSuperProperty;
use super::{BytecodeAddress, BytecodeCallSite, BytecodeDirectThrow, BytecodeTemplateElement};
use crate::{
    api::native_call::NativeCallTarget,
    bytecode::BytecodeHoistPlan,
    error::{Error, Result},
    syntax::{
        AccessorKind, BinaryOp, DeclKind, FunctionKind, ImportPhase, StaticFunctionId, StaticName,
        StaticPropertyAccessId, StaticString, UnaryOp, UpdateOp,
    },
    value::Value,
};
use std::rc::Rc;

mod binding;
pub use binding::BytecodeBinding;

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeProgram {
    block: BytecodeBlock,
    hoist_plan: BytecodeHoistPlan,
}

impl BytecodeProgram {
    pub(crate) const fn new(block: BytecodeBlock, hoist_plan: BytecodeHoistPlan) -> Self {
        Self { block, hoist_plan }
    }
    pub const fn block(&self) -> &BytecodeBlock {
        &self.block
    }
    pub const fn hoist_plan(&self) -> &BytecodeHoistPlan {
        &self.hoist_plan
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BytecodeObjectProperty {
    Static(StaticName),
    StaticMethod(StaticName),
    StaticAccessor { key: StaticName, kind: AccessorKind },
    Computed,
    ComputedInferredName,
    ComputedMethod,
    ComputedAccessor { kind: AccessorKind },
    Spread,
}

impl BytecodeObjectProperty {
    pub const fn stack_value_count(&self) -> usize {
        match self {
            Self::Static(_)
            | Self::StaticMethod(_)
            | Self::StaticAccessor { .. }
            | Self::Spread => 1,
            Self::Computed
            | Self::ComputedInferredName
            | Self::ComputedMethod
            | Self::ComputedAccessor { .. } => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeFunctionDeclaration {
    name: BytecodeBinding,
    id: StaticFunctionId,
    function_name: StaticName,
    bytecode: BytecodeFunction,
    kind: FunctionKind,
}

impl BytecodeFunctionDeclaration {
    pub(crate) const fn new(
        name: BytecodeBinding,
        id: StaticFunctionId,
        function_name: StaticName,
        bytecode: BytecodeFunction,
        kind: FunctionKind,
    ) -> Self {
        Self {
            name,
            id,
            function_name,
            bytecode,
            kind,
        }
    }

    pub const fn name(&self) -> &BytecodeBinding {
        &self.name
    }

    pub const fn id(&self) -> StaticFunctionId {
        self.id
    }

    pub const fn function_name(&self) -> &StaticName {
        &self.function_name
    }

    pub const fn bytecode(&self) -> &BytecodeFunction {
        &self.bytecode
    }

    pub const fn kind(&self) -> FunctionKind {
        self.kind
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BytecodeProperty {
    name: StaticName,
    access: StaticPropertyAccessId,
}

impl BytecodeProperty {
    pub(crate) const fn new(name: StaticName, access: StaticPropertyAccessId) -> Self {
        Self { name, access }
    }

    pub const fn name(&self) -> &StaticName {
        &self.name
    }

    pub const fn access(&self) -> StaticPropertyAccessId {
        self.access
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BytecodeArrayIndex {
    index: u32,
}

impl BytecodeArrayIndex {
    const INDEX_LIMIT: u32 = u32::MAX;

    pub(crate) fn parse(property: &BytecodeProperty) -> Option<Self> {
        let index = property.name().as_str().parse::<u32>().ok()?;
        if index == Self::INDEX_LIMIT || index.to_string() != property.name().as_str() {
            return None;
        }
        Some(Self { index })
    }

    pub(crate) fn index(self) -> Result<usize> {
        usize::try_from(self.index)
            .map_err(|_| Error::limit("array index exceeded supported range"))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BytecodeDynamicProperty {
    access: StaticPropertyAccessId,
}

impl BytecodeDynamicProperty {
    pub(crate) const fn new(access: StaticPropertyAccessId) -> Self {
        Self { access }
    }

    pub const fn access(self) -> StaticPropertyAccessId {
        self.access
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BytecodeForInTarget {
    Binding {
        name: BytecodeBinding,
        kind: DeclKind,
    },
    PatternBinding {
        pattern: Rc<BytecodePattern>,
        kind: DeclKind,
    },
    PatternAssignment(Rc<BytecodePattern>),
    Assignment(BytecodeAssignmentTarget),
}

/// Compiled class literal: the constructor function plus prototype and
/// static members. Computed member keys are evaluated onto the stack in
/// member order before `CreateClass` runs.
#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeClass {
    pub name: Option<StaticName>,
    /// Immutable inner class-name binding initialized after heritage evaluation.
    pub inner_name_binding: Option<BytecodeBinding>,
    /// When true the heritage value sits on the stack below computed keys.
    pub heritage: bool,
    pub constructor_id: StaticFunctionId,
    pub default_derived_constructor: bool,
    pub constructor: BytecodeFunction,
    pub members: Rc<[BytecodeClassMember]>,
    pub fields: Rc<[BytecodeClassField]>,
    pub static_blocks: Rc<[BytecodeBlock]>,
    pub static_element_order: Rc<[BytecodeClassStaticElement]>,
    /// Declared `#name` identifiers in declaration order; class evaluation
    /// allocates one fresh runtime private name per entry.
    pub private_names: Rc<[StaticName]>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BytecodeClassStaticElement {
    Field(usize),
    Block(usize),
}

/// A compiled class field with a lazily evaluated initializer block.
#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeClassField {
    pub key: BytecodeClassMemberKey,
    pub is_static: bool,
    pub name: Option<StaticName>,
    pub infer_name_from_computed_key: bool,
    pub initializer: Option<BytecodeBlock>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeClassMember {
    pub key: BytecodeClassMemberKey,
    pub kind: BytecodeClassMemberKind,
    pub function_kind: FunctionKind,
    pub is_static: bool,
    pub id: StaticFunctionId,
    pub bytecode: BytecodeFunction,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BytecodeClassMemberKind {
    Method,
    Getter,
    Setter,
}

/// Compiled destructuring binding pattern. Default initializers and computed
/// keys stay as lazily evaluated blocks so they only run when the runtime
/// walker needs them.
#[derive(Debug, Clone, PartialEq)]
pub enum BytecodePattern {
    Binding(BytecodeBinding),
    Assignment(BytecodeAssignmentTarget),
    Object {
        properties: Rc<[BytecodePatternProperty]>,
        rest: Option<Rc<Self>>,
    },
    Array {
        /// `None` entries are elisions that consume one iterator step.
        elements: Rc<[Option<BytecodePatternTarget>]>,
        rest: Option<Rc<Self>>,
    },
}

impl BytecodePattern {
    pub(crate) fn for_each_binding(
        &self,
        visit: &mut impl FnMut(&BytecodeBinding) -> Result<()>,
    ) -> Result<()> {
        match self {
            Self::Binding(binding) => visit(binding),
            Self::Assignment(_) => Err(Error::runtime(
                "assignment target appeared in a binding pattern",
            )),
            Self::Object { properties, rest } => {
                for property in properties.iter() {
                    property.target.pattern.for_each_binding(visit)?;
                }
                if let Some(rest) = rest {
                    rest.for_each_binding(visit)?;
                }
                Ok(())
            }
            Self::Array { elements, rest } => {
                for element in elements.iter().flatten() {
                    element.pattern.for_each_binding(visit)?;
                }
                if let Some(rest) = rest {
                    rest.for_each_binding(visit)?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BytecodeDestructureMode {
    Declaration(DeclKind),
    Parameter,
    Assignment,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodePatternProperty {
    pub key: BytecodePatternKey,
    pub target: BytecodePatternTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BytecodePatternKey {
    Static(StaticName),
    Computed(BytecodeBlock),
}

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodePatternTarget {
    pub pattern: BytecodePattern,
    pub default: Option<BytecodeBlock>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BytecodeAssignmentTarget {
    Binding(BytecodeBinding),
    WebCompatCall(BytecodeBlock),
    StaticProperty {
        object: BytecodeBlock,
        property: BytecodeProperty,
        strict: bool,
    },
    ArrayIndexProperty {
        object: BytecodeBlock,
        property: BytecodeProperty,
        index: BytecodeArrayIndex,
        strict: bool,
    },
    ComputedProperty {
        object: BytecodeBlock,
        property: BytecodeBlock,
        operand: BytecodeDynamicProperty,
        strict: bool,
    },
    PrivateProperty {
        object: BytecodeBlock,
        property: BytecodePrivateName,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeSwitchCase {
    pub test: Option<BytecodeBlock>,
    pub body: BytecodeBlock,
    pub body_updates_value: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeCatch {
    pub param: Option<Rc<BytecodePattern>>,
    pub param_bindings: Rc<[BytecodeBinding]>,
    pub body: BytecodeBlock,
    pub body_scoped: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BytecodeInstruction {
    BeginPrivateEnvironment {
        names: Rc<[StaticName]>,
    },
    PushLiteral(Value),
    PushString(StaticString),
    TemplateConcat {
        part_count: usize,
    },
    GetTemplateObject {
        site: BytecodeCallSite,
        quasis: Rc<[BytecodeTemplateElement]>,
    },
    StringConcat {
        final_result: bool,
    },
    StringConcatStatic {
        text: StaticString,
        final_result: bool,
    },
    CollectSpreadArgs {
        spread_flags: Rc<[bool]>,
    },
    CallBindingSpread {
        callee: BytecodeBinding,
        native: Option<NativeCallTarget>,
        strict: bool,
    },
    CallValueSpread,
    CallValueWithReceiverSpread,
    CallStaticMemberSpread {
        property: BytecodeProperty,
    },
    CallComputedMemberSpread {
        property: BytecodeDynamicProperty,
    },
    ConstructValueSpread,
    ArrayLiteralSpread {
        spread_flags: Rc<[bool]>,
        holes: Rc<[bool]>,
    },
    CreateRegExp {
        pattern: StaticString,
        flags: StaticString,
    },
    DynamicImport {
        phase: ImportPhase,
        specifier: BytecodeBlock,
        options: Option<BytecodeBlock>,
    },
    PushUndefined,
    LoadThis,
    ImportMeta,
    LoadNewTarget,
    LoadBinding(BytecodeBinding),
    StoreBinding(BytecodeBinding),
    StoreAnnexBVar(StaticName),
    HoistLexicalBinding {
        name: BytecodeBinding,
        kind: DeclKind,
    },
    ResolveBinding(BytecodeBinding),
    StoreResolvedBinding(BytecodeBinding),
    DeclareBinding {
        name: BytecodeBinding,
        kind: DeclKind,
        has_init: bool,
    },
    StoreLast,
    Pop,
    Duplicate,
    Unary(UnaryOp),
    NumberUnary(BytecodeNumericUnaryOp),
    Await,
    GeneratorStart,
    Yield {
        delegate: bool,
    },
    NullishCoalescing {
        right: BytecodeBlock,
    },
    TypeOfBinding(BytecodeBinding),
    TypeOfValue,
    ToPropertyKey,
    DeleteBinding(BytecodeBinding),
    DeleteStaticProperty {
        property: BytecodeProperty,
        strict: bool,
    },
    DeleteComputedProperty {
        property: BytecodeDynamicProperty,
        strict: bool,
    },
    DeleteSuperProperty,
    DeleteValue,
    UpdateBinding {
        name: BytecodeBinding,
        op: UpdateOp,
        prefix: bool,
    },
    UpdateStaticProperty {
        property: BytecodeProperty,
        op: UpdateOp,
        prefix: bool,
        strict: bool,
    },
    UpdateArrayIndexProperty {
        property: BytecodeProperty,
        index: BytecodeArrayIndex,
        op: UpdateOp,
        prefix: bool,
        strict: bool,
    },
    UpdateComputedProperty {
        property: BytecodeDynamicProperty,
        op: UpdateOp,
        prefix: bool,
        strict: bool,
    },
    Binary {
        op: BinaryOp,
        property_access: Option<BytecodeDynamicProperty>,
    },
    InStaticProperty {
        property: StaticString,
        access: BytecodeDynamicProperty,
    },
    NumberBinary(BytecodeNumericBinaryOp),
    NumberCompare(BytecodeNumericCompareOp),
    NumberEquality(BytecodeNumericEqualityOp),
    CompoundStoreBinding {
        name: BytecodeBinding,
        op: BinaryOp,
    },
    CompoundStaticProperty {
        property: BytecodeProperty,
        op: BinaryOp,
        strict: bool,
    },
    CompoundArrayIndexProperty {
        property: BytecodeProperty,
        index: BytecodeArrayIndex,
        op: BinaryOp,
        strict: bool,
    },
    CompoundComputedProperty {
        property: BytecodeDynamicProperty,
        op: BinaryOp,
        strict: bool,
    },
    LogicalAssignment {
        op: BinaryOp,
        target: BytecodeAssignmentTarget,
        value: BytecodeBlock,
    },
    WebCompatCallAssignment {
        target: BytecodeBlock,
    },
    StaticMember {
        property: BytecodeProperty,
    },
    OptionalStaticMember {
        property: BytecodeProperty,
    },
    /// Reads a private slot with brand and accessor semantics.
    PrivateMember {
        property: BytecodePrivateName,
    },
    /// Writes a private slot with brand and accessor semantics.
    PrivateAssign {
        property: BytecodePrivateName,
    },
    /// Reads, combines, and writes one private slot for `obj.#x op= value`.
    CompoundPrivateProperty {
        property: BytecodePrivateName,
        op: BinaryOp,
    },
    /// Applies `++`/`--` to one private slot, pushing the pre- or post-value.
    UpdatePrivateProperty {
        property: BytecodePrivateName,
        op: UpdateOp,
        prefix: bool,
    },
    /// Calls one private method or accessor result with the object receiver.
    CallPrivateMember {
        property: BytecodePrivateName,
        arg_count: usize,
    },
    /// Spread-argument variant of `CallPrivateMember`.
    CallPrivateMemberSpread {
        property: BytecodePrivateName,
    },
    /// Pushes whether the popped object owns the resolved private slot.
    PrivateIn {
        property: BytecodePrivateName,
    },
    ArrayLength {
        property: BytecodeProperty,
    },
    ArrayIndexMember {
        property: BytecodeProperty,
        index: BytecodeArrayIndex,
    },
    ComputedMember {
        property: BytecodeDynamicProperty,
    },
    StaticPropertyAssign {
        property: BytecodeProperty,
        strict: bool,
    },
    ArrayIndexAssign {
        property: BytecodeProperty,
        index: BytecodeArrayIndex,
        strict: bool,
    },
    ComputedPropertyAssign {
        property: BytecodeDynamicProperty,
        strict: bool,
    },
    CallBinding {
        callee: BytecodeBinding,
        native: Option<NativeCallTarget>,
        strict: bool,
        arg_count: usize,
    },
    TailCallBinding {
        callee: BytecodeBinding,
        native: Option<NativeCallTarget>,
        strict: bool,
        arg_count: usize,
    },
    CallValue {
        site: BytecodeCallSite,
        arg_count: usize,
    },
    CallValueWithReceiver {
        site: BytecodeCallSite,
        arg_count: usize,
    },
    TailCallValue {
        arg_count: usize,
    },
    CallStaticMember {
        property: BytecodeProperty,
        native: Option<NativeCallTarget>,
        arg_count: usize,
    },
    CallComputedMember {
        property: BytecodeDynamicProperty,
        native: Option<NativeCallTarget>,
        arg_count: usize,
    },
    Construct {
        constructor: BytecodeBinding,
        native: Option<NativeCallTarget>,
        arg_count: usize,
    },
    ConstructValue {
        native: Option<NativeCallTarget>,
        arg_count: usize,
    },
    CreateFunction {
        id: StaticFunctionId,
        name: Option<StaticName>,
        bytecode: BytecodeFunction,
        constructable: bool,
        kind: FunctionKind,
        new_target_mode: BytecodeNewTargetMode,
    },
    ArrayLiteral {
        len: usize,
        holes: Rc<[bool]>,
    },
    ObjectLiteral {
        properties: Rc<[BytecodeObjectProperty]>,
    },
    While {
        labels: Option<Rc<[StaticName]>>,
        condition: BytecodeBlock,
        body: BytecodeBlock,
    },
    DoWhile {
        labels: Option<Rc<[StaticName]>>,
        body: BytecodeBlock,
        condition: BytecodeBlock,
    },
    With {
        body: BytecodeBlock,
    },
    For {
        labels: Option<Rc<[StaticName]>>,
        init: Option<BytecodeBlock>,
        condition: Option<BytecodeBlock>,
        update: Option<BytecodeBlock>,
        body: BytecodeBlock,
        scoped: bool,
        per_iteration: bool,
    },
    ForIn {
        labels: Option<Rc<[StaticName]>>,
        target: BytecodeForInTarget,
        object: BytecodeBlock,
        body: BytecodeBlock,
    },
    ForOf {
        labels: Option<Rc<[StaticName]>>,
        target: BytecodeForInTarget,
        object: BytecodeBlock,
        body: BytecodeBlock,
        asynchronous: bool,
    },
    DestructurePattern {
        pattern: Rc<BytecodePattern>,
        mode: BytecodeDestructureMode,
    },
    CreateClass {
        class: Rc<BytecodeClass>,
    },
    CallSuper {
        arg_count: usize,
    },
    CallSuperSpread,
    SuperMember {
        property: BytecodeProperty,
    },
    ComputedSuperMember {
        expression: BytecodeBlock,
        property: BytecodeDynamicProperty,
    },
    CallSuperMember {
        property: BytecodeProperty,
        arg_count: usize,
    },
    CallSuperMemberSpread {
        property: BytecodeProperty,
    },
    CallComputedSuperMember {
        property: BytecodeDynamicProperty,
        arg_count: usize,
    },
    CallComputedSuperMemberSpread {
        property: BytecodeDynamicProperty,
    },
    SuperPropertyAssign {
        property: BytecodeSuperProperty,
        value: BytecodeBlock,
        strict: bool,
    },
    UpdateSuperProperty {
        property: BytecodeSuperProperty,
        op: UpdateOp,
        prefix: bool,
        strict: bool,
    },
    CompoundSuperProperty {
        property: BytecodeSuperProperty,
        op: BinaryOp,
        value: BytecodeBlock,
        strict: bool,
    },
    Switch {
        discriminant: BytecodeBlock,
        cases: Rc<[BytecodeSwitchCase]>,
        scoped: bool,
        scope_init: Option<BytecodeBlock>,
    },
    Try {
        body: BytecodeBlock,
        body_scoped: bool,
        body_direct_throw: Option<BytecodeDirectThrow>,
        catch: Option<BytecodeCatch>,
        finally_body: Option<BytecodeBlock>,
        finally_scoped: bool,
    },
    Label {
        label: StaticName,
        body: BytecodeBlock,
    },
    ScopedBlock {
        block: BytecodeBlock,
        var_hoist_plan: Option<Rc<BytecodeHoistPlan>>,
        preserve_last: bool,
        push_result: bool,
    },
    Jump(BytecodeAddress),
    JumpIfFalse(BytecodeAddress),
    JumpIfFalseKeep(BytecodeAddress),
    JumpIfTrueKeep(BytecodeAddress),
    JumpIfNullishKeep(BytecodeAddress),
    Complete(BytecodeCompletion),
}
