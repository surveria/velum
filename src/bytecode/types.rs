use std::{fmt, rc::Rc};

use crate::{
    api::native_call::NativeCallTarget,
    ast::{
        BinaryOp, DeclKind, FunctionParam, StaticBinding, StaticCallSiteId, StaticFunctionId,
        StaticName, StaticPropertyAccessId, StaticString, UnaryOp, UpdateOp,
    },
    binding_layout::{BindingLayout, BindingOperand},
    bytecode::BytecodeHoistPlan,
    error::{Error, Result},
    value::{ErrorName, Value},
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
    param_defaults: Rc<[Option<BytecodeBlock>]>,
    body: BytecodeBlock,
    hoist_plan: BytecodeHoistPlan,
    capture_bindings: Rc<[StaticBinding]>,
}

impl BytecodeFunction {
    pub(crate) const fn new(
        param_defaults: Rc<[Option<BytecodeBlock>]>,
        body: BytecodeBlock,
        hoist_plan: BytecodeHoistPlan,
        capture_bindings: Rc<[StaticBinding]>,
    ) -> Self {
        Self {
            param_defaults,
            body,
            hoist_plan,
            capture_bindings,
        }
    }

    pub fn param_defaults(&self) -> &[Option<BytecodeBlock>] {
        &self.param_defaults
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
    params: Rc<[FunctionParam]>,
    bytecode: BytecodeFunction,
    is_async: bool,
}

impl BytecodeFunctionDeclaration {
    pub(crate) const fn new(
        name: BytecodeBinding,
        id: StaticFunctionId,
        function_name: StaticName,
        params: Rc<[FunctionParam]>,
        bytecode: BytecodeFunction,
        is_async: bool,
    ) -> Self {
        Self {
            name,
            id,
            function_name,
            params,
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

    pub const fn params(&self) -> &Rc<[FunctionParam]> {
        &self.params
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

    pub(super) fn instructions(&self) -> &[BytecodeInstruction] {
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BytecodeNumericBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    BitAnd,
    BitOr,
    BitXor,
    ShiftLeft,
    ShiftRight,
    ShiftRightUnsigned,
}

impl BytecodeNumericBinaryOp {
    pub(crate) const fn from_binary(op: BinaryOp) -> Option<Self> {
        match op {
            BinaryOp::Add => Some(Self::Add),
            BinaryOp::Sub => Some(Self::Sub),
            BinaryOp::Mul => Some(Self::Mul),
            BinaryOp::Div => Some(Self::Div),
            BinaryOp::Rem => Some(Self::Rem),
            BinaryOp::Pow => Some(Self::Pow),
            BinaryOp::BitAnd => Some(Self::BitAnd),
            BinaryOp::BitOr => Some(Self::BitOr),
            BinaryOp::BitXor => Some(Self::BitXor),
            BinaryOp::ShiftLeft => Some(Self::ShiftLeft),
            BinaryOp::ShiftRight => Some(Self::ShiftRight),
            BinaryOp::ShiftRightUnsigned => Some(Self::ShiftRightUnsigned),
            BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::StrictEqual
            | BinaryOp::StrictNotEqual
            | BinaryOp::Less
            | BinaryOp::LessEqual
            | BinaryOp::Greater
            | BinaryOp::GreaterEqual
            | BinaryOp::In
            | BinaryOp::LogicalAnd
            | BinaryOp::LogicalOr => None,
        }
    }

    pub(crate) const fn fallback_binary(self) -> BinaryOp {
        match self {
            Self::Add => BinaryOp::Add,
            Self::Sub => BinaryOp::Sub,
            Self::Mul => BinaryOp::Mul,
            Self::Div => BinaryOp::Div,
            Self::Rem => BinaryOp::Rem,
            Self::Pow => BinaryOp::Pow,
            Self::BitAnd => BinaryOp::BitAnd,
            Self::BitOr => BinaryOp::BitOr,
            Self::BitXor => BinaryOp::BitXor,
            Self::ShiftLeft => BinaryOp::ShiftLeft,
            Self::ShiftRight => BinaryOp::ShiftRight,
            Self::ShiftRightUnsigned => BinaryOp::ShiftRightUnsigned,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BytecodeNumericUnaryOp {
    Negate,
    Plus,
}

impl BytecodeNumericUnaryOp {
    pub(crate) const fn from_unary(op: UnaryOp) -> Option<Self> {
        match op {
            UnaryOp::Negate => Some(Self::Negate),
            UnaryOp::Plus => Some(Self::Plus),
            UnaryOp::Not | UnaryOp::Void | UnaryOp::Typeof | UnaryOp::Delete => None,
        }
    }

    pub(crate) const fn fallback_unary(self) -> UnaryOp {
        match self {
            Self::Negate => UnaryOp::Negate,
            Self::Plus => UnaryOp::Plus,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BytecodeNumericCompareOp {
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

impl BytecodeNumericCompareOp {
    pub(crate) const fn from_binary(op: BinaryOp) -> Option<Self> {
        match op {
            BinaryOp::Less => Some(Self::Less),
            BinaryOp::LessEqual => Some(Self::LessEqual),
            BinaryOp::Greater => Some(Self::Greater),
            BinaryOp::GreaterEqual => Some(Self::GreaterEqual),
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Rem
            | BinaryOp::Pow
            | BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::StrictEqual
            | BinaryOp::StrictNotEqual
            | BinaryOp::In
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::ShiftLeft
            | BinaryOp::ShiftRight
            | BinaryOp::ShiftRightUnsigned
            | BinaryOp::LogicalAnd
            | BinaryOp::LogicalOr => None,
        }
    }

    pub(crate) const fn fallback_binary(self) -> BinaryOp {
        match self {
            Self::Less => BinaryOp::Less,
            Self::LessEqual => BinaryOp::LessEqual,
            Self::Greater => BinaryOp::Greater,
            Self::GreaterEqual => BinaryOp::GreaterEqual,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BytecodeNumericEqualityOp {
    Equal,
    NotEqual,
    StrictEqual,
    StrictNotEqual,
}

impl BytecodeNumericEqualityOp {
    pub(crate) const fn from_binary(op: BinaryOp) -> Option<Self> {
        match op {
            BinaryOp::Equal => Some(Self::Equal),
            BinaryOp::NotEqual => Some(Self::NotEqual),
            BinaryOp::StrictEqual => Some(Self::StrictEqual),
            BinaryOp::StrictNotEqual => Some(Self::StrictNotEqual),
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Rem
            | BinaryOp::Pow
            | BinaryOp::Less
            | BinaryOp::LessEqual
            | BinaryOp::Greater
            | BinaryOp::GreaterEqual
            | BinaryOp::In
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::ShiftLeft
            | BinaryOp::ShiftRight
            | BinaryOp::ShiftRightUnsigned
            | BinaryOp::LogicalAnd
            | BinaryOp::LogicalOr => None,
        }
    }

    pub(crate) const fn fallback_binary(self) -> BinaryOp {
        match self {
            Self::Equal => BinaryOp::Equal,
            Self::NotEqual => BinaryOp::NotEqual,
            Self::StrictEqual => BinaryOp::StrictEqual,
            Self::StrictNotEqual => BinaryOp::StrictNotEqual,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BytecodeForInTarget {
    Binding {
        name: BytecodeBinding,
        kind: DeclKind,
    },
    Assignment(BytecodeAssignmentTarget),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BytecodeAssignmentTarget {
    Binding(BytecodeBinding),
    StaticProperty {
        object: BytecodeBlock,
        property: BytecodeProperty,
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
    CompoundComputedProperty {
        property: BytecodeDynamicProperty,
        op: BinaryOp,
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
    CreateFunction {
        id: crate::ast::StaticFunctionId,
        name: Option<StaticName>,
        params: Rc<[FunctionParam]>,
        bytecode: BytecodeFunction,
        constructable: bool,
        is_async: bool,
        new_target_mode: BytecodeNewTargetMode,
    },
    ArrayLiteral {
        len: usize,
    },
    ObjectLiteral {
        properties: Rc<[StaticName]>,
    },
    If {
        condition: BytecodeBlock,
        consequent: BytecodeBlock,
        alternate: Option<BytecodeBlock>,
    },
    While {
        condition: BytecodeBlock,
        body: BytecodeBlock,
    },
    For {
        init: Option<BytecodeBlock>,
        condition: Option<BytecodeBlock>,
        update: Option<BytecodeBlock>,
        body: BytecodeBlock,
        scoped: bool,
    },
    ForIn {
        target: BytecodeForInTarget,
        object: BytecodeBlock,
        body: BytecodeBlock,
    },
    Switch {
        discriminant: BytecodeBlock,
        cases: Rc<[BytecodeSwitchCase]>,
    },
    Try {
        body: BytecodeBlock,
        catch: Option<BytecodeCatch>,
        finally_body: Option<BytecodeBlock>,
    },
    ScopedBlock(BytecodeBlock),
    Jump(BytecodeAddress),
    JumpIfFalse(BytecodeAddress),
    JumpIfFalseKeep(BytecodeAddress),
    JumpIfTrueKeep(BytecodeAddress),
    Complete(BytecodeCompletion),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BytecodeCompletion {
    Break,
    Continue,
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
