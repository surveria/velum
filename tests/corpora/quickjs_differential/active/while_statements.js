let values = [10, 20, 10, 2];
let index = 0;
let total = 0;

while (index < values.length) {
    total = total + values[index];
    index = index + 1;
}

print(index);
print(total);

while (false) {
    var hoisted = 42;
}
print(hoisted);

let pick = function() {
    let value = 0;
    while (value < 4) {
        value = value + 1;
        if (value === 2) {
            return 42;
        }
    }
    return 0;
};

print(pick());
