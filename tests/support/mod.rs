use rs_quickjs::Context;

const ASSERT_HARNESS: &str = include_str!("../harness/assert.js");

pub fn install_assert(context: &mut Context) -> rs_quickjs::Result<()> {
    context.eval(ASSERT_HARNESS).map(|_| ())
}
