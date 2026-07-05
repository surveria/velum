let object = {
    name: "front-door",
    first: 40,
    nested: { value: 2 },
    duplicate: 1,
    duplicate: 41,
};

print(object.name);
print(object.first + object.nested.value);
print(object.missing);
print(object.duplicate);

let assigned = object.first = 42;
print(assigned);
print(object.first);

let shared = {};
print(shared === shared);
print(shared === {});

let make = function() {
    let state = { value: 40 };
    return function() {
        state.value = state.value + 1;
        return state.value;
    };
};

let next = make();
print(next());
print(next());
