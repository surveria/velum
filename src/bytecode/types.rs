use std::{fmt, rc::Rc};

use crate::{
    ast::{
        BinaryOp, DeclKind, StaticBinding, StaticName, StaticPropertyAccessId, StaticString,
        UnaryOp, UpdateOp,
    },
    binding_layout::{BindingLayout, BindingOperand},
    bytecode::BytecodeHoistPlan,
    error::{Error, Result},
    native_call::NativeCallTarget,
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
    body: BytecodeBlock,
    hoist_plan: BytecodeHoistPlan,
    capture_bindings: Rc<[StaticBinding]>,
}

impl BytecodeFunction {
    pub(crate) const fn new(
        body: BytecodeBlock,
        hoist_plan: BytecodeHoistPlan,
        capture_bindings: Rc<[StaticBinding]>,
    ) -> Self {
        Self {
            body,
            hoist_plan,
            capture_bindings,
        }
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
pub enum BytecodeNumericBinaryOp {
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
}

impl BytecodeNumericBinaryOp {
    pub(crate) const fn from_binary(op: BinaryOp) -> Option<Self> {
        match op {
            BinaryOp::Sub => Some(Self::Sub),
            BinaryOp::Mul => Some(Self::Mul),
            BinaryOp::Div => Some(Self::Div),
            BinaryOp::Rem => Some(Self::Rem),
            BinaryOp::Pow => Some(Self::Pow),
            BinaryOp::Add
            | BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::StrictEqual
            | BinaryOp::StrictNotEqual
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
            Self::Sub => BinaryOp::Sub,
            Self::Mul => BinaryOp::Mul,
            Self::Div => BinaryOp::Div,
            Self::Rem => BinaryOp::Rem,
            Self::Pow => BinaryOp::Pow,
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
    ComputedMember {
        property: BytecodeDynamicProperty,
    },
    StaticPropertyAssign {
        property: BytecodeProperty,
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
        arg_count: usize,
    },
    CallStaticMember {
        property: BytecodeProperty,
        native: Option<NativeCallTarget>,
        arg_count: usize,
    },
    CallComputedMember {
        property: BytecodeDynamicProperty,
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
        arg_count: usize,
    },
    CreateFunction {
        id: crate::ast::StaticFunctionId,
        name: Option<StaticName>,
        params: Rc<[StaticBinding]>,
        bytecode: BytecodeFunction,
        constructable: bool,
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
