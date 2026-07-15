use velum::Context;

const ASSERT_HARNESS: &str = include_str!("../harness/assert.js");

pub fn install_assert(context: &mut Context) -> velum::Result<()> {
    context.eval(ASSERT_HARNESS).map(|_| ())
}
