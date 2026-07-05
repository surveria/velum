let global = 1;
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
let result = add(40, 2);
let absent = missing();
let selected = first(7, ignored = 99);
let duplicate_result = duplicate(1, 2);
assert.throws(ReferenceError, function() {
    local = local;
});
global = global + 41;
print(result);
print(absent);
print(selected);
print(ignored);
print(duplicate_result);
print(global);
result
