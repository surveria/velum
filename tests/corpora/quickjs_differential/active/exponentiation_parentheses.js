let power = 2 ** 3 ** 2;
let grouped = (-2) ** 2;
let negated = -(2 ** 2);
print(power, grouped, negated);

let value = 2;
let assigned = (value) **= 3;
let old = (value)++;
let current = ++(value);
print(assigned, old, current, value);

let target = { slot: 4 };
let propPower = (target.slot) **= 2;
print(propPower, target.slot);

let missingType = typeof (missing);
let deleteMissing = delete (missing);
let object = { value: 1 };
let deleteProperty = delete (object.value);
print(missingType, deleteMissing, deleteProperty, object.value);

let choose = function(value) {
    return value;
};
let called = (choose)(42);
print(called);
