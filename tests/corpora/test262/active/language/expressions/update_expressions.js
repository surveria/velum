let value = 40;
let first = value++;
let second = ++value;
let third = value--;
let fourth = --value;

if (first !== 40 || second !== 42 || third !== 42 || fourth !== 40 || value !== 40) {
    throw new Test262Error("identifier update expressions produced unexpected values");
}

let sensor = { count: 1 };
let propOld = sensor.count++;
let propNew = ++sensor.count;

if (propOld !== 1 || propNew !== 3 || sensor.count !== 3) {
    throw new Test262Error("property update expressions produced unexpected values");
}

let values = [1, 2];
let index = 0;
let cellOld = values[index]++;
let cellNew = ++values[1];

if (cellOld !== 1 || cellNew !== 3 || values[0] !== 2 || values[1] !== 3) {
    throw new Test262Error("computed property update expressions produced unexpected values");
}

let total = 0;
for (let step = 0; step < 4; step++) {
    total = total + step;
}

let down = 2;
while (down--) {}

if (total !== 6 || down !== -1) {
    throw new Test262Error("loop update expressions produced unexpected values");
}

42
