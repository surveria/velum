let outer = 1;
let total = 0;
{
    let outer = 40;
    const delta = 2;
    total = outer + delta;
    print(total, typeof delta);
}
print(outer, typeof delta);

let loopTotal = 0;
for (let index = 0; index < 4; index = index + 1) {
    let record = { value: index + 1 };
    loopTotal = loopTotal + record.value;
}
print(loopTotal, typeof index, typeof record);

let pair = 0;
for (let left = 20, right = 22; left < 21; left = left + 1) {
    pair = left + right;
}
print(pair, typeof left, typeof right);

{
    var hoisted = 42;
}
print(hoisted);

let status = "";
try {
    let hidden = 1;
    throw "boom";
} catch (error) {
    let caught = 40;
    status = error + " " + caught;
} finally {
    let finalValue = 2;
    status = status + " " + finalValue;
}
print(status, typeof hidden, typeof error, typeof caught, typeof finalValue);
