use crate::{
    ast::{Expr, Expression, StaticCallSiteId},
    bytecode::{BytecodeBlock, BytecodeCallSite, BytecodePrivateName},
    error::Result,
};

use super::{BytecodeCompiler, BytecodeInstruction, InstructionIndex, has_spread_arg};

#[derive(Clone, Copy)]
struct OptionalChainExit {
    jump: InstructionIndex,
    pop_count: usize,
}

impl BytecodeCompiler<'_> {
    pub(super) fn compile_optional_chain(&mut self, expression: &Expression) -> Result<()> {
        let mut exits = Vec::new();
        self.compile_optional_chain_part(expression, &mut exits)?;
        self.finish_optional_chain_exits(exits, 1)
    }

    fn finish_optional_chain_exits(
        &mut self,
        exits: Vec<OptionalChainExit>,
        undefined_count: usize,
    ) -> Result<()> {
        let normal_end = self.emit_jump();
        let mut branch_ends = Vec::with_capacity(exits.len());
        for exit in exits {
            let branch_address = self.current_address();
            self.patch_jump(exit.jump, branch_address)?;
            for _ in 0..exit.pop_count {
                self.emit(BytecodeInstruction::Pop);
            }
            for _ in 0..undefined_count {
                self.emit(BytecodeInstruction::PushUndefined);
            }
            branch_ends.push(self.emit_jump());
        }
        let end_address = self.current_address();
        self.patch_jump(normal_end, end_address)?;
        for branch_end in branch_ends {
            self.patch_jump(branch_end, end_address)?;
        }
        Ok(())
    }

    pub(super) fn compile_parenthesized_optional_chain_call(
        &mut self,
        expression: &Expression,
        site: StaticCallSiteId,
        args: &[Expression],
    ) -> Result<()> {
        let mut exits = Vec::new();
        let has_receiver = self.compile_chain_call_target(expression, &mut exits)?;
        self.finish_optional_chain_exits(exits, usize::from(has_receiver).saturating_add(1))?;
        if has_spread_arg(args) {
            let spread_flags = self.compile_spread_parts(args)?;
            self.emit(BytecodeInstruction::CollectSpreadArgs { spread_flags });
            self.emit(if has_receiver {
                BytecodeInstruction::CallValueWithReceiverSpread
            } else {
                BytecodeInstruction::CallValueSpread
            });
            return Ok(());
        }
        self.compile_args(args)?;
        self.emit(if has_receiver {
            BytecodeInstruction::CallValueWithReceiver {
                site: BytecodeCallSite::new(site),
                arg_count: args.len(),
            }
        } else {
            BytecodeInstruction::CallValue {
                site: BytecodeCallSite::new(site),
                arg_count: args.len(),
            }
        });
        Ok(())
    }

    fn compile_optional_chain_part(
        &mut self,
        expression: &Expression,
        exits: &mut Vec<OptionalChainExit>,
    ) -> Result<()> {
        match expression.kind() {
            Expr::Member {
                object,
                property,
                access,
            } => {
                self.compile_optional_chain_part(object, exits)?;
                self.emit(BytecodeInstruction::StaticMember {
                    property: Self::compile_property(property, *access),
                });
            }
            Expr::OptionalMember {
                object,
                property,
                access,
            } => {
                self.compile_optional_chain_part(object, exits)?;
                self.record_optional_chain_exit(exits, 1);
                self.emit(BytecodeInstruction::StaticMember {
                    property: Self::compile_property(property, *access),
                });
            }
            Expr::ComputedMember {
                object,
                property,
                access,
            } => {
                self.compile_optional_chain_part(object, exits)?;
                self.compile_expr(property)?;
                self.emit(BytecodeInstruction::ComputedMember {
                    property: Self::compile_dynamic_property(*access),
                });
            }
            Expr::OptionalComputedMember {
                object,
                property,
                access,
            } => {
                self.compile_optional_chain_part(object, exits)?;
                self.record_optional_chain_exit(exits, 1);
                self.compile_expr(property)?;
                self.emit(BytecodeInstruction::ComputedMember {
                    property: Self::compile_dynamic_property(*access),
                });
            }
            Expr::PrivateMember { object, name } => {
                self.compile_optional_chain_part(object, exits)?;
                self.emit(BytecodeInstruction::PrivateMember {
                    property: BytecodePrivateName::new(name.clone()),
                });
            }
            Expr::OptionalPrivateMember { object, name } => {
                self.compile_optional_chain_part(object, exits)?;
                self.record_optional_chain_exit(exits, 1);
                self.emit(BytecodeInstruction::PrivateMember {
                    property: BytecodePrivateName::new(name.clone()),
                });
            }
            Expr::Call {
                callee, site, args, ..
            } => self.compile_chain_call(callee, *site, args, false, exits)?,
            Expr::OptionalCall {
                callee, site, args, ..
            } => self.compile_chain_call(callee, *site, args, true, exits)?,
            _ => self.compile_expr(expression)?,
        }
        Ok(())
    }

    fn compile_chain_call(
        &mut self,
        callee: &Expression,
        site: StaticCallSiteId,
        args: &[Expression],
        optional: bool,
        exits: &mut Vec<OptionalChainExit>,
    ) -> Result<()> {
        let has_receiver = self.compile_chain_call_target(callee, exits)?;
        if optional {
            self.record_optional_chain_exit(exits, usize::from(has_receiver).saturating_add(1));
        }
        if has_spread_arg(args) {
            let spread_flags = self.compile_spread_parts(args)?;
            self.emit(BytecodeInstruction::CollectSpreadArgs { spread_flags });
            self.emit(if has_receiver {
                BytecodeInstruction::CallValueWithReceiverSpread
            } else {
                BytecodeInstruction::CallValueSpread
            });
            return Ok(());
        }
        self.compile_args(args)?;
        self.emit(if has_receiver {
            BytecodeInstruction::CallValueWithReceiver {
                site: BytecodeCallSite::new(site),
                arg_count: args.len(),
            }
        } else {
            BytecodeInstruction::CallValue {
                site: BytecodeCallSite::new(site),
                arg_count: args.len(),
            }
        });
        Ok(())
    }

    fn compile_chain_call_target(
        &mut self,
        callee: &Expression,
        exits: &mut Vec<OptionalChainExit>,
    ) -> Result<bool> {
        match callee.kind() {
            Expr::Member {
                object,
                property,
                access,
            } => {
                self.compile_optional_chain_part(object, exits)?;
                self.emit(BytecodeInstruction::Duplicate);
                self.emit(BytecodeInstruction::StaticMember {
                    property: Self::compile_property(property, *access),
                });
                Ok(true)
            }
            Expr::OptionalMember {
                object,
                property,
                access,
            } => {
                self.compile_optional_chain_part(object, exits)?;
                self.record_optional_chain_exit(exits, 1);
                self.emit(BytecodeInstruction::Duplicate);
                self.emit(BytecodeInstruction::StaticMember {
                    property: Self::compile_property(property, *access),
                });
                Ok(true)
            }
            Expr::ComputedMember {
                object,
                property,
                access,
            } => {
                self.compile_optional_chain_part(object, exits)?;
                self.emit(BytecodeInstruction::Duplicate);
                self.compile_expr(property)?;
                self.emit(BytecodeInstruction::ComputedMember {
                    property: Self::compile_dynamic_property(*access),
                });
                Ok(true)
            }
            Expr::OptionalComputedMember {
                object,
                property,
                access,
            } => {
                self.compile_optional_chain_part(object, exits)?;
                self.record_optional_chain_exit(exits, 1);
                self.emit(BytecodeInstruction::Duplicate);
                self.compile_expr(property)?;
                self.emit(BytecodeInstruction::ComputedMember {
                    property: Self::compile_dynamic_property(*access),
                });
                Ok(true)
            }
            Expr::PrivateMember { object, name } => {
                self.compile_optional_chain_part(object, exits)?;
                self.emit(BytecodeInstruction::Duplicate);
                self.emit(BytecodeInstruction::PrivateMember {
                    property: BytecodePrivateName::new(name.clone()),
                });
                Ok(true)
            }
            Expr::OptionalPrivateMember { object, name } => {
                self.compile_optional_chain_part(object, exits)?;
                self.record_optional_chain_exit(exits, 1);
                self.emit(BytecodeInstruction::Duplicate);
                self.emit(BytecodeInstruction::PrivateMember {
                    property: BytecodePrivateName::new(name.clone()),
                });
                Ok(true)
            }
            Expr::SuperMember { property, access } => {
                self.emit(BytecodeInstruction::LoadThis);
                self.emit(BytecodeInstruction::SuperMember {
                    property: Self::compile_property(property, *access),
                });
                Ok(true)
            }
            Expr::SuperComputedMember { property, access } => {
                self.emit(BytecodeInstruction::LoadThis);
                self.emit(BytecodeInstruction::ComputedSuperMember {
                    expression: BytecodeBlock::compile_expression(property, self.layout)?,
                    property: Self::compile_dynamic_property(*access),
                });
                Ok(true)
            }
            Expr::Parenthesized(callee) => self.compile_chain_call_target(callee, exits),
            Expr::OptionalChain(callee) => {
                let mut nested_exits = Vec::new();
                let has_receiver = self.compile_chain_call_target(callee, &mut nested_exits)?;
                self.finish_optional_chain_exits(
                    nested_exits,
                    usize::from(has_receiver).saturating_add(1),
                )?;
                Ok(has_receiver)
            }
            _ => {
                self.compile_optional_chain_part(callee, exits)?;
                Ok(false)
            }
        }
    }

    fn record_optional_chain_exit(&mut self, exits: &mut Vec<OptionalChainExit>, pop_count: usize) {
        exits.push(OptionalChainExit {
            jump: self.emit_jump_if_nullish_keep(),
            pop_count,
        });
    }
}
