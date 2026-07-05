let total = 0;

for (let i = 0; i < 150; i = i + 1) {
    let f = function namedCamera(a, b) {};
    Object.defineProperty(f, "name", {
        value: "patched",
        writable: true,
        enumerable: true,
        configurable: true
    });
    f.name = "assigned";
    Object.defineProperty(f, "length", {
        value: i,
        writable: true,
        configurable: true
    });
    f.length = f.length + 1;
    total = total + f.length + Object.keys(f).length;
    delete f.name;
    delete f.length;
    f.length = i;
    total = total + f.length;

    Object.defineProperty(TypeError, "name", {
        value: "Typed",
        writable: true,
        configurable: true
    });
    TypeError.name = "TypedAssigned";
    total = total + TypeError.name.length;
    delete TypeError.name;
}

total
