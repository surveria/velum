let total = 0;

for (let i = 0; i < 250; i = i + 1) {
    let f = function namedCamera(a, b) {};
    Object.defineProperty(f, "tag", {
        value: i,
        enumerable: true,
        writable: false,
        configurable: false
    });
    let descriptor = Object.getOwnPropertyDescriptor(f, "tag");
    let keys = Object.keys(f);
    if (Object.hasOwn(f, "tag")) {
        total = total + descriptor.value + keys.length + f.length;
    }
}

total
