let total = 0;

for (let index = 0; index < 65536; index = index + 1) {
  let object = {
    first: index,
    second: index + 1,
    nested: { value: index + 2 },
  };
  object.first = object.first + object.second;
  object.second = object.first + object.nested.value;
  object.nested.value = object.second + object.first;
  total = total + object.first + object.second + object.nested.value;
}

total;
