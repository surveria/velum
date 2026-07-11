(async function() {
  async function seed(value = 40) {
    return value + 1;
  }

  function answer(left, right = left + 1,) {
    if (answer.length !== 1) {
      return 0;
    }
    return right;
  }

  return answer(await seed(undefined));
})().then(function(value) {
  print("default-parameters:" + value);
}, function(error) {
  print("default-parameters-error:" + error);
});

42
