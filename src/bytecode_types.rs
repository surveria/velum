use std::rc::Rc;

use crate::{
    ast::{
        BinaryOp, DeclKind, StaticBinding, StaticName, StaticPropertyAccessId, StaticString,
        UnaryOp, UpdateOp,
    },
    bytecode_hoist::BytecodeHoistPlan,
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

    pub fn instruction_count(&self) -> usize {
        self.block.instruction_count()
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

    pub fn instruction_count(&self) -> usize {
        self.body.instruction_count()
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

    pub fn instruction_count(&self) -> usize {
        let nested = self
            .instructions
            .iter()
            .map(BytecodeInstruction::nested_instruction_count)
            .sum::<usize>();
        self.instructions.len().saturating_add(nested)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BytecodeForInTarget {
    Binding { name: StaticBinding, kind: DeclKind },
    Assignment(BytecodeAssignmentTarget),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BytecodeAssignmentTarget {
    Binding(StaticBinding),
    StaticProperty {
        object: BytecodeBlock,
        property: StaticName,
        access: StaticPropertyAccessId,
    },
    ComputedProperty {
        object: BytecodeBlock,
        property: BytecodeBlock,
        access: StaticPropertyAccessId,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeSwitchCase {
    pub test: Option<BytecodeBlock>,
    pub body: BytecodeBlock,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeCatch {
    pub param: Option<StaticBinding>,
    pub body: BytecodeBlock,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BytecodeInstruction {
    PushLiteral(Value),
    PushString(StaticString),
    PushUndefined,
    LoadThis,
    LoadBinding(StaticBinding),
    StoreBinding(StaticBinding),
    DeclareBinding {
        name: StaticBinding,
        kind: DeclKind,
        has_init: bool,
    },
    StoreLast,
    Pop,
    Unary(UnaryOp),
    TypeOfBinding(StaticBinding),
    TypeOfValue,
    DeleteBinding(StaticBinding),
    DeleteStaticProperty {
        property: StaticName,
    },
    DeleteComputedProperty,
    DeleteValue,
    UpdateBinding {
        name: StaticBinding,
        op: UpdateOp,
        prefix: bool,
    },
    UpdateStaticProperty {
        property: StaticName,
        access: StaticPropertyAccessId,
        op: UpdateOp,
        prefix: bool,
    },
    UpdateComputedProperty {
        access: StaticPropertyAccessId,
        op: UpdateOp,
        prefix: bool,
    },
    Binary {
        op: BinaryOp,
        property_access: Option<StaticPropertyAccessId>,
    },
    CompoundStoreBinding {
        name: StaticBinding,
        op: BinaryOp,
    },
    CompoundStaticProperty {
        property: StaticName,
        access: StaticPropertyAccessId,
        op: BinaryOp,
    },
    CompoundComputedProperty {
        access: StaticPropertyAccessId,
        op: BinaryOp,
    },
    StaticMember {
        property: StaticName,
        access: StaticPropertyAccessId,
    },
    ComputedMember {
        access: StaticPropertyAccessId,
    },
    StaticPropertyAssign {
        property: StaticName,
        access: StaticPropertyAccessId,
    },
    ComputedPropertyAssign {
        access: StaticPropertyAccessId,
    },
    CallBinding {
        callee: StaticBinding,
        arg_count: usize,
    },
    CallValue {
        arg_count: usize,
    },
    CallStaticMember {
        property: StaticName,
        access: StaticPropertyAccessId,
        arg_count: usize,
    },
    CallComputedMember {
        access: StaticPropertyAccessId,
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
        constructor: StaticBinding,
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

impl BytecodeInstruction {
    fn nested_instruction_count(&self) -> usize {
        match self {
            Self::If {
                condition,
                consequent,
                alternate,
            } => condition
                .instruction_count()
                .saturating_add(consequent.instruction_count())
                .saturating_add(
                    alternate
                        .as_ref()
                        .map_or(0, BytecodeBlock::instruction_count),
                ),
            Self::While { condition, body } => condition
                .instruction_count()
                .saturating_add(body.instruction_count()),
            Self::For {
                init,
                condition,
                update,
                body,
                ..
            } => init
                .as_ref()
                .map_or(0, BytecodeBlock::instruction_count)
                .saturating_add(
                    condition
                        .as_ref()
                        .map_or(0, BytecodeBlock::instruction_count),
                )
                .saturating_add(update.as_ref().map_or(0, BytecodeBlock::instruction_count))
                .saturating_add(body.instruction_count()),
            Self::ForIn {
                object,
                body,
                target,
            } => object
                .instruction_count()
                .saturating_add(body.instruction_count())
                .saturating_add(target.nested_instruction_count()),
            Self::Switch {
                discriminant,
                cases,
            } => discriminant.instruction_count().saturating_add(
                cases
                    .iter()
                    .map(BytecodeSwitchCase::instruction_count)
                    .sum::<usize>(),
            ),
            Self::Try {
                body,
                catch,
                finally_body,
            } => body
                .instruction_count()
                .saturating_add(catch.as_ref().map_or(0, BytecodeCatch::instruction_count))
                .saturating_add(
                    finally_body
                        .as_ref()
                        .map_or(0, BytecodeBlock::instruction_count),
                ),
            Self::ScopedBlock(block) => block.instruction_count(),
            Self::CreateFunction { bytecode, .. } => bytecode.instruction_count(),
            instruction if instruction.is_leaf_instruction() => 0,
            _ => 0,
        }
    }

    const fn is_leaf_instruction(&self) -> bool {
        matches!(
            self,
            Self::DeleteBinding(_)
                | Self::DeleteStaticProperty { .. }
                | Self::DeleteComputedProperty
                | Self::DeleteValue
                | Self::UpdateBinding { .. }
                | Self::UpdateStaticProperty { .. }
                | Self::UpdateComputedProperty { .. }
                | Self::CompoundStoreBinding { .. }
                | Self::CompoundStaticProperty { .. }
                | Self::CompoundComputedProperty { .. }
                | Self::CallBinding { .. }
                | Self::CallValue { .. }
                | Self::CallStaticMember { .. }
                | Self::CallComputedMember { .. }
                | Self::Print { .. }
                | Self::AssertThrows { .. }
                | Self::Construct { .. }
                | Self::PushLiteral(_)
                | Self::PushString(_)
                | Self::PushUndefined
                | Self::LoadThis
                | Self::LoadBinding(_)
                | Self::StoreBinding(_)
                | Self::DeclareBinding { .. }
                | Self::StoreLast
                | Self::Pop
                | Self::Unary(_)
                | Self::TypeOfBinding(_)
                | Self::TypeOfValue
                | Self::Binary { .. }
                | Self::StaticMember { .. }
                | Self::ComputedMember { .. }
                | Self::StaticPropertyAssign { .. }
                | Self::ComputedPropertyAssign { .. }
                | Self::ArrayLiteral { .. }
                | Self::ObjectLiteral { .. }
                | Self::Jump(_)
                | Self::JumpIfFalse(_)
                | Self::JumpIfFalseKeep(_)
                | Self::JumpIfTrueKeep(_)
                | Self::Complete(_)
        )
    }
}

impl BytecodeForInTarget {
    fn nested_instruction_count(&self) -> usize {
        match self {
            Self::Binding { .. } => 0,
            Self::Assignment(target) => target.nested_instruction_count(),
        }
    }
}

impl BytecodeAssignmentTarget {
    fn nested_instruction_count(&self) -> usize {
        match self {
            Self::Binding(_) => 0,
            Self::StaticProperty { object, .. } => object.instruction_count(),
            Self::ComputedProperty {
                object, property, ..
            } => object
                .instruction_count()
                .saturating_add(property.instruction_count()),
        }
    }
}

impl BytecodeSwitchCase {
    fn instruction_count(&self) -> usize {
        self.test
            .as_ref()
            .map_or(0, BytecodeBlock::instruction_count)
            .saturating_add(self.body.instruction_count())
    }
}

impl BytecodeCatch {
    fn instruction_count(&self) -> usize {
        self.body.instruction_count()
    }
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
