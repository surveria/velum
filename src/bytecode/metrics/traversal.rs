use crate::bytecode::{BytecodeBlock, BytecodeCatch, BytecodeSwitchCase};

pub(super) fn count_blocks_2(
    first: &BytecodeBlock,
    second: &BytecodeBlock,
    count: fn(&BytecodeBlock) -> usize,
) -> usize {
    count(first).saturating_add(count(second))
}

pub(super) fn count_blocks_3(
    first: &BytecodeBlock,
    second: &BytecodeBlock,
    third: Option<&BytecodeBlock>,
    count: fn(&BytecodeBlock) -> usize,
) -> usize {
    count_blocks_2(first, second, count).saturating_add(third.map_or(0, count))
}

pub(super) fn count_for_blocks(
    init: Option<&BytecodeBlock>,
    condition: Option<&BytecodeBlock>,
    update: Option<&BytecodeBlock>,
    body: &BytecodeBlock,
    count: fn(&BytecodeBlock) -> usize,
) -> usize {
    init.map_or(0, count)
        .saturating_add(condition.map_or(0, count))
        .saturating_add(update.map_or(0, count))
        .saturating_add(count(body))
}

pub(super) fn count_switch(
    discriminant: &BytecodeBlock,
    cases: &[BytecodeSwitchCase],
    block_count: fn(&BytecodeBlock) -> usize,
    case_count: fn(&BytecodeSwitchCase) -> usize,
) -> usize {
    block_count(discriminant).saturating_add(cases.iter().map(case_count).sum::<usize>())
}

pub(super) fn count_try(
    body: &BytecodeBlock,
    catch: Option<&BytecodeCatch>,
    finally_body: Option<&BytecodeBlock>,
    block_count: fn(&BytecodeBlock) -> usize,
    catch_count: fn(&BytecodeCatch) -> usize,
) -> usize {
    block_count(body)
        .saturating_add(catch.map_or(0, catch_count))
        .saturating_add(finally_body.map_or(0, block_count))
}
