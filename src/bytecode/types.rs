use std::{fmt, rc::Rc};

use crate::{
    api::native_call::NativeCallTarget,
    binding_metadata::{BindingLayout, BindingOperand},
    bytecode::BytecodeHoistPlan,
    error::{Error, Result},
    syntax::{
        AccessorKind, BinaryOp, DeclKind, StaticBinding, StaticCallSiteId, StaticFunctionId,
        StaticName, StaticPropertyAccessId, StaticString, UnaryOp, UpdateOp,
    },
    value::{ErrorName, Value},
};

use super::numeric::{
    BytecodeNumericBinaryOp, BytecodeNumericCompareOp, BytecodeNumericEqualityOp,
    BytecodeNumericUnaryOp,
};

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

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeFunction {
    params: Rc<[BytecodeFunctionParam]>,
    param_defaults: Rc<[Option<BytecodeBlock>]>,
    body: BytecodeBlock,
    hoist_plan: BytecodeHoistPlan,
    capture_bindings: Rc<[StaticBinding]>,
}

impl BytecodeFunction {
    pub(crate) const fn new(
        params: Rc<[BytecodeFunctionParam]>,
        param_defaults: Rc<[Option<BytecodeBlock>]>,
        body: BytecodeBlock,
        hoist_plan: BytecodeHoistPlan,
        capture_bindings: Rc<[StaticBinding]>,
    ) -> Self {
        Self {
            params,
            param_defaults,
            body,
            hoist_plan,
            capture_bindings,
        }
    }

    pub fn params(&self) -> &[BytecodeFunctionParam] {
        &self.params
    }

    pub fn param_defaults(&self) -> &[Option<BytecodeBlock>] {
        &self.param_defaults
    }

    pub fn has_parameter_defaults(&self) -> bool {
        self.param_defaults.iter().any(Option::is_some)
    }

    pub const fn body(&self) -> &BytecodeBlock {
        &self.body
    }

    pub const fn hoist_plan(&self) -> &BytecodeHoistPlan {
        &self.hoist_plan
    }

    pub fn capture_bindings(&self) -> &[StaticBinding] {
        &self.capture_bindings
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BytecodeFunctionParam {
    binding: StaticBinding,
    has_default: bool,
    rest: bool,
}

impl BytecodeFunctionParam {
    pub(crate) const fn new(binding: StaticBinding, has_default: bool, rest: bool) -> Self {
        Self {
            binding,
            has_default,
            rest,
        }
    }

    pub const fn binding(&self) -> &StaticBinding {
        &self.binding
    }

    pub const fn has_default(&self) -> bool {
        self.has_default
    }

    pub const fn rest(&self) -> bool {
        self.rest
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BytecodeObjectProperty {
    Static(StaticName),
    StaticAccessor { key: StaticName, kind: AccessorKind },
    Computed,
    ComputedMethod,
    ComputedAccessor { kind: AccessorKind },
    Spread,
}

impl BytecodeObjectProperty {
    pub const fn stack_value_count(&self) -> usize {
        match self {
            Self::Static(_) | Self::StaticAccessor { .. } | Self::Spread => 1,
            Self::Computed | Self::ComputedMethod | Self::ComputedAccessor { .. } => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BytecodeNewTargetMode {
    Own,
    Lexical,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeFunctionDeclaration {
    name: BytecodeBinding,
    id: StaticFunctionId,
    function_name: StaticName,
    bytecode: BytecodeFunction,
    is_async: bool,
}

impl BytecodeFunctionDeclaration {
    pub(crate) const fn new(
        name: BytecodeBinding,
        id: StaticFunctionId,
        function_name: StaticName,
        bytecode: BytecodeFunction,
        is_async: bool,
    ) -> Self {
        Self {
            name,
            id,
            function_name,
            bytecode,
            is_async,
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

    pub const fn is_async(&self) -> bool {
        self.is_async
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeBlock {
    instructions: Rc<[BytecodeInstruction]>,
}

impl BytecodeBlock {
    pub(crate) fn from_instructions(instructions: Vec<BytecodeInstruction>) -> Self {
        Self {
            instructions: Rc::from(instructions.into_boxed_slice()),
        }
    }

    pub fn instruction(&self, address: BytecodeAddress) -> Result<Option<&BytecodeInstruction>> {
        let index = address.index();
        if index == self.instructions.len() {
            return Ok(None);
        }
        if index > self.instructions.len() {
            return Err(Error::runtime(
                "bytecode instruction pointer escaped program",
            ));
        }
        Ok(self.instructions.get(index))
    }

    pub(crate) fn instructions(&self) -> &[BytecodeInstruction] {
        &self.instructions
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BytecodeBinding {
    name: StaticBinding,
    operand: BindingOperand,
}

impl BytecodeBinding {
    pub(crate) fn compile(name: &StaticBinding, layout: &BindingLayout) -> Result<Self> {
        let operand = layout
            .operand_for_binding_id(name.id())?
            .unwrap_or(BindingOperand::Unresolved);
        Ok(Self {
            name: name.clone(),
            operand,
        })
    }

    pub const fn name(&self) -> &StaticBinding {
        &self.name
    }

    pub const fn operand(&self) -> BindingOperand {
        self.operand
    }
}

impl fmt::Display for BytecodeBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.fmt(formatter)
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
    Assignment(BytecodeAssignmentTarget),
}

/// Compiled class literal: the constructor function plus prototype and
/// static members. Computed member keys are evaluated onto the stack in
/// member order before `CreateClass` runs.
#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeClass {
    pub name: Option<StaticName>,
    /// When true the heritage value sits on the stack below computed keys.
    pub heritage: bool,
    pub constructor_id: StaticFunctionId,
    pub constructor: BytecodeFunction,
    pub members: Rc<[BytecodeClassMember]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeClassMember {
    pub key: BytecodeClassMemberKey,
    pub kind: BytecodeClassMemberKind,
    pub is_static: bool,
    pub id: StaticFunctionId,
    pub name: Option<StaticName>,
    pub bytecode: BytecodeFunction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BytecodeClassMemberKey {
    Static(StaticName),
    Computed,
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
    Object {
        properties: Rc<[BytecodePatternProperty]>,
        rest: Option<BytecodeBinding>,
    },
    Array {
        /// `None` entries are elisions that consume one iterator step.
        elements: Rc<[Option<BytecodePatternTarget>]>,
        rest: Option<Rc<Self>>,
    },
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
    StaticProperty {
        object: BytecodeBlock,
        property: BytecodeProperty,
    },
    ArrayIndexProperty {
        object: BytecodeBlock,
        property: BytecodeProperty,
        index: BytecodeArrayIndex,
    },
    ComputedProperty {
        object: BytecodeBlock,
        property: BytecodeBlock,
        operand: BytecodeDynamicProperty,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeSwitchCase {
    pub test: Option<BytecodeBlock>,
    pub body: BytecodeBlock,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeCatch {
    pub param: Option<BytecodeBinding>,
    pub body: BytecodeBlock,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BytecodeInstruction {
    PushLiteral(Value),
    PushString(StaticString),
    TemplateConcat {
        part_count: usize,
    },
    CollectSpreadArgs {
        spread_flags: Rc<[bool]>,
    },
    CallBindingSpread {
        callee: BytecodeBinding,
    },
    CallValueSpread,
    CallStaticMemberSpread {
        property: BytecodeProperty,
    },
    CallComputedMemberSpread {
        property: BytecodeDynamicProperty,
    },
    ConstructValueSpread,
    ArrayLiteralSpread {
        spread_flags: Rc<[bool]>,
    },
    CreateRegExp {
        pattern: StaticString,
        flags: StaticString,
    },
    PushUndefined,
    LoadThis,
    LoadNewTarget,
    LoadBinding(BytecodeBinding),
    StoreBinding(BytecodeBinding),
    DeclareBinding {
        name: BytecodeBinding,
        kind: DeclKind,
        has_init: bool,
    },
    StoreLast,
    Pop,
    Unary(UnaryOp),
    NumberUnary(BytecodeNumericUnaryOp),
    Await,
    NullishCoalescing {
        right: BytecodeBlock,
    },
    TypeOfBinding(BytecodeBinding),
    TypeOfValue,
    DeleteBinding(BytecodeBinding),
    DeleteStaticProperty {
        property: BytecodeProperty,
    },
    DeleteComputedProperty {
        property: BytecodeDynamicProperty,
    },
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
    },
    UpdateArrayIndexProperty {
        property: BytecodeProperty,
        index: BytecodeArrayIndex,
        op: UpdateOp,
        prefix: bool,
    },
    UpdateComputedProperty {
        property: BytecodeDynamicProperty,
        op: UpdateOp,
        prefix: bool,
    },
    Binary {
        op: BinaryOp,
        property_access: Option<BytecodeDynamicProperty>,
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
    },
    CompoundArrayIndexProperty {
        property: BytecodeProperty,
        index: BytecodeArrayIndex,
        op: BinaryOp,
    },
    CompoundComputedProperty {
        property: BytecodeDynamicProperty,
        op: BinaryOp,
    },
    LogicalAssignment {
        op: BinaryOp,
        target: BytecodeAssignmentTarget,
        value: BytecodeBlock,
    },
    StaticMember {
        property: BytecodeProperty,
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
    },
    ArrayIndexAssign {
        property: BytecodeProperty,
        index: BytecodeArrayIndex,
    },
    ComputedPropertyAssign {
        property: BytecodeDynamicProperty,
    },
    CallBinding {
        callee: BytecodeBinding,
        native: Option<NativeCallTarget>,
        arg_count: usize,
    },
    CallValue {
        site: BytecodeCallSite,
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
    Print {
        arg_count: usize,
    },
    AssertThrows {
        expected: ErrorName,
        has_message: bool,
    },
    Construct {
        constructor: BytecodeBinding,
        native: Option<NativeCallTarget>,
        arg_count: usize,
    },
    ConstructValue {
        arg_count: usize,
    },
    CreateFunction {
        id: StaticFunctionId,
        name: Option<StaticName>,
        bytecode: BytecodeFunction,
        constructable: bool,
        is_async: bool,
        new_target_mode: BytecodeNewTargetMode,
    },
    ArrayLiteral {
        len: usize,
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
    For {
        labels: Option<Rc<[StaticName]>>,
        init: Option<BytecodeBlock>,
        condition: Option<BytecodeBlock>,
        update: Option<BytecodeBlock>,
        body: BytecodeBlock,
        scoped: bool,
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
    },
    DestructurePattern {
        pattern: Rc<BytecodePattern>,
        kind: DeclKind,
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
    CallSuperMember {
        property: BytecodeProperty,
        arg_count: usize,
    },
    CallSuperMemberSpread {
        property: BytecodeProperty,
    },
    Switch {
        discriminant: BytecodeBlock,
        cases: Rc<[BytecodeSwitchCase]>,
        scoped: bool,
    },
    Try {
        body: BytecodeBlock,
        catch: Option<BytecodeCatch>,
        finally_body: Option<BytecodeBlock>,
    },
    Label {
        label: StaticName,
        body: BytecodeBlock,
    },
    ScopedBlock(BytecodeBlock),
    Jump(BytecodeAddress),
    JumpIfFalse(BytecodeAddress),
    JumpIfFalseKeep(BytecodeAddress),
    JumpIfTrueKeep(BytecodeAddress),
    Complete(BytecodeCompletion),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BytecodeCompletion {
    Break(Option<StaticName>),
    Continue(Option<StaticName>),
    Return,
    Throw,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BytecodeAddress(usize);

impl BytecodeAddress {
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    pub const fn index(self) -> usize {
        self.0
    }
}
