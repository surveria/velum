let value = 40;
let first = value++;
let second = ++value;
let third = value--;
let fourth = --value;
print(first, second, third, fourth, value);

let sensor = { count: 1 };
let propOld = sensor.count++;
let propNew = ++sensor.count;
print(propOld, propNew, sensor.count);

let values = [1, 2];
let index = 0;
let cellOld = values[index]++;
let cellNew = ++values[1];
print(cellOld, cellNew, values[0], values[1]);

let total = 0;
for (let step = 0; step < 4; step++) {
    total = total + step;
}
let down = 2;
while (down--) {}
print(total, down);

first === 40 &&
second === 42 &&
third === 42 &&
fourth === 40 &&
value === 40 &&
propOld === 1 &&
propNew === 3 &&
sensor.count === 3 &&
cellOld === 1 &&
cellNew === 3 &&
values[0] === 2 &&
values[1] === 3 &&
total === 6 &&
down === -1 ? 42 : 0
