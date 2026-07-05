let object = {
  first: 1,
  second: 2,
  third: 3,
  fourth: 4,
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
object["fourth"];
