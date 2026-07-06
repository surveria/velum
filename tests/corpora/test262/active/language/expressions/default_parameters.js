async function seed(value = 40) {
  return value + 1;
}

function answer(left, right = left + 1,) {
  if (answer.length !== 1) {
    return 0;
  }
  return right;
}

answer(await seed(undefined));
