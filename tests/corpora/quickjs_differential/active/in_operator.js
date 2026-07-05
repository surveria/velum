let object = { present: 1, empty: undefined };
let present = "present" in object;
let empty = "empty" in object;
let absent = "absent" in object;
print(present, empty, absent);

delete object.present;
let deleted = "present" in object;
print(deleted);

let values = [undefined, 2];
values[3] = 4;
let hasZero = 0 in values;
let hasOne = "1" in values;
let hasTwo = 2 in values;
let hasThree = 3 in values;
let hasLength = "length" in values;
print(hasZero, hasOne, hasTwo, hasThree, hasLength);

let key = "slot";
let bag = { slot: 42 };
let precedence = key in bag === true;
print(key in bag, ("slot") in bag, precedence);
