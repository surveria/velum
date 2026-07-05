let object = {
  first: 1,
  second: 2,
  nested: { value: 3 },
};

object.first = object.first + object.second;
object.second = object.first + object.nested.value;
object.nested.value = object.second + object.first;
object.first + object.second + object.nested.value;
