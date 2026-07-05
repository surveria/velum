let choose = function() {
  try {
    return 40 + 2;
  } catch (error) {
    return 0;
  }
  return 1;
};

let empty = function() {
  return;
  return 1;
};

if (empty() !== undefined) {
  throw new Test262Error("empty return mismatch");
}

choose();
