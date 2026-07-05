let object = { first: 1, second: 2, third: 3 };
delete object.second;
object.second = 20;

let seen = "";
for (let key in object) {
    seen = seen + key + ":" + object[key] + ";";
}
print(seen, typeof key);

let values = [10, 20];
values[3] = 40;
let indexes = "";
for (const index in values) {
    indexes = indexes + index + "=" + values[index] + ";";
}
print(indexes, typeof index);

var hoisted = "start";
for (var name in { alpha: 1, beta: 2 }) {
    hoisted = name;
}
print(hoisted, typeof name, name);

let target = { slot: "" };
let selected = "";
for (target.slot in { a: 1, b: 2, c: 3 }) {
    if (target.slot === "b") {
        continue;
    }
    selected = selected + target.slot;
    if (target.slot === "c") {
        break;
    }
}
print(selected, target.slot);
