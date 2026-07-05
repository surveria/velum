let value = 10;
let add = value += 5;
let sub = value -= 3;
let mul = value *= 4;
let div = value /= 2;
let rem = value %= 7;
let mask = value &= 6;
print(add, sub, mul, div, rem, mask, value);

let label = "cam";
label += "-01";
print(label);

let sensor = { count: 10 };
let propAdd = sensor.count += 5;
let propSub = sensor.count -= 3;
print(propAdd, propSub, sensor.count);

let values = [1, 2, 3];
let index = 1;
let cellMul = values[index] *= 5;
let cellBit = values[index] &= 6;
print(cellMul, cellBit, values[1]);

let order = "";
let target = { slot: 40 };
let key = function() {
    order += "k";
    return "slot";
};
let rhs = function() {
    order += "r";
    return 2;
};
let ordered = target[key()] += rhs();
print(order, ordered, target.slot);

add === 15 &&
    sub === 12 &&
    mul === 48 &&
    div === 24 &&
    rem === 3 &&
    mask === 2 &&
    value === 2 &&
    label === "cam-01" &&
    propAdd === 15 &&
    propSub === 12 &&
    sensor.count === 12 &&
    cellMul === 10 &&
    cellBit === 2 &&
    values[1] === 2 &&
    order === "kr" &&
    ordered === 42 &&
    target.slot === 42 ? 42 : 0
