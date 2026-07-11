(async () => {
  let plusOne = value => value + 1;

  let answer = async (left, right,) => {
    let base = await Promise.resolve(left);
    return plusOne(base + right);
  };

  let first = await answer(39, 2);
  if (first !== 42) {
    throw new Test262Error("async arrow result mismatch");
  }

  let defaultAnswer = async (left = 40, right = 2,) => {
    let base = await Promise.resolve(left);
    return plusOne(base + right - 1);
  };

  return await defaultAnswer(undefined);
})().then(function(value) {
  print("async-arrow:" + value);
}, function(error) {
  print("async-arrow-error:" + error);
});

42
