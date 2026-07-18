use crate::ExecutionError;

const HIGH_SURROGATE_START: u16 = 0xD800;
const HIGH_SURROGATE_END: u16 = 0xDBFF;
const LOW_SURROGATE_START: u16 = 0xDC00;
const LOW_SURROGATE_END: u16 = 0xDFFF;
const SUPPLEMENTARY_OFFSET: u32 = 0x1_0000;

pub fn decode_forward(
    input: &[u16],
    position: usize,
    unicode: bool,
) -> Result<Option<(u32, usize)>, ExecutionError> {
    let Some(first) = input.get(position).copied() else {
        return Ok(None);
    };
    let single_end = position
        .checked_add(1)
        .ok_or(ExecutionError::SizeOverflow)?;
    if !unicode || !(HIGH_SURROGATE_START..=HIGH_SURROGATE_END).contains(&first) {
        return Ok(Some((u32::from(first), single_end)));
    }
    let Some(second) = input.get(single_end).copied() else {
        return Ok(Some((u32::from(first), single_end)));
    };
    if !(LOW_SURROGATE_START..=LOW_SURROGATE_END).contains(&second) {
        return Ok(Some((u32::from(first), single_end)));
    }
    let high = u32::from(first - HIGH_SURROGATE_START);
    let low = u32::from(second - LOW_SURROGATE_START);
    let shifted = high
        .checked_mul(0x400)
        .ok_or(ExecutionError::SizeOverflow)?;
    let scalar = SUPPLEMENTARY_OFFSET
        .checked_add(shifted)
        .and_then(|value| value.checked_add(low))
        .ok_or(ExecutionError::SizeOverflow)?;
    let pair_end = single_end
        .checked_add(1)
        .ok_or(ExecutionError::SizeOverflow)?;
    Ok(Some((scalar, pair_end)))
}

pub fn advance_candidate(
    input: &[u16],
    position: usize,
    unicode: bool,
) -> Result<usize, ExecutionError> {
    decode_forward(input, position, unicode)?
        .map(|(_, next)| next)
        .ok_or(ExecutionError::StartOutOfBounds)
}

pub fn decode_backward(
    input: &[u16],
    position: usize,
    unicode: bool,
) -> Result<Option<u32>, ExecutionError> {
    Ok(decode_backward_with_position(input, position, unicode)?.map(|(value, _)| value))
}

pub fn decode_backward_with_position(
    input: &[u16],
    position: usize,
    unicode: bool,
) -> Result<Option<(u32, usize)>, ExecutionError> {
    let Some(previous_position) = position.checked_sub(1) else {
        return Ok(None);
    };
    let Some(last) = input.get(previous_position).copied() else {
        return Err(ExecutionError::InvalidProgram);
    };
    if !unicode || !(LOW_SURROGATE_START..=LOW_SURROGATE_END).contains(&last) {
        return Ok(Some((u32::from(last), previous_position)));
    }
    let Some(high_position) = previous_position.checked_sub(1) else {
        return Ok(Some((u32::from(last), previous_position)));
    };
    let Some(first) = input.get(high_position).copied() else {
        return Err(ExecutionError::InvalidProgram);
    };
    if !(HIGH_SURROGATE_START..=HIGH_SURROGATE_END).contains(&first) {
        return Ok(Some((u32::from(last), previous_position)));
    }
    let high = u32::from(first - HIGH_SURROGATE_START);
    let low = u32::from(last - LOW_SURROGATE_START);
    let scalar = SUPPLEMENTARY_OFFSET
        .checked_add(
            high.checked_mul(0x400)
                .ok_or(ExecutionError::SizeOverflow)?,
        )
        .and_then(|value| value.checked_add(low))
        .ok_or(ExecutionError::SizeOverflow)?;
    Ok(Some((scalar, high_position)))
}

pub const fn is_line_terminator(value: u16) -> bool {
    matches!(value, 0x000A | 0x000D | 0x2028 | 0x2029)
}
