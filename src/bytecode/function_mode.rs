#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BytecodeNewTargetMode {
    Own,
    Lexical,
}
