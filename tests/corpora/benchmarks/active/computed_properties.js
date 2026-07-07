let total = 0;

for (let index = 0; index < 16384; index = index + 1) {
  let object = {
    first: index,
    second: index + 1,
    third: index + 2,
    fourth: index + 3,
  };
  let key = "first";
  object[key] = object[key] + object["second"];
  object[key] = object[key] + object["third"];
  object[key] = object[key] + object["fourth"];
  object[1] = object[key];
  object[true] = object["1"] + object[key];
  object[key] = object[true] + object["second"];
  object["second"] = object[key] + object[1];
  object["third"] = object["second"] + object[true];
  object["fourth"] = object["third"] + object["second"];
  total = total + object["fourth"];
}

total;
