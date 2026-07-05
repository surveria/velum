let value = 1;
let add = function(left, right) {
    var local = left + right;
    return local;
};
let missing = function(value) {
    return value;
};
let first = function(value) {
    return value;
};
let duplicate = function(value, value) {
    return value;
};
let ignored = 0;

print(add(20, 22));
print(missing());
print(first(7, ignored = 42));
print(ignored);
print(duplicate(1, 2));

try {
    local = local;
} catch (error) {
    print(error.name);
}

print(value);
