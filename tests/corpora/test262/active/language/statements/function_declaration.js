let before = declared(20, 22);

function declared(left, right) {
  return left + right;
}

function recursive(value) {
  if (value <= 1) {
    return 1;
  }
  return value * recursive(value - 1);
}

function outer(base) {
  return inner(2);

  function inner(value) {
    return base + value;
  }
}

if (before !== 42) {
  throw new Test262Error("function declaration hoist mismatch");
}

if (recursive(5) !== 120) {
  throw new Test262Error("recursive function declaration mismatch");
}

outer(40);
