let total = 0;

for (let i = 0; i < 250; i = i + 1) {
    let object = { base: i };
    Object.defineProperty(object, "visible", {
        value: i + 1,
        enumerable: true,
        writable: false,
        configurable: false
    });
    Object.defineProperty(object, "hidden", { value: i + 2 });
    let descriptor = Object.getOwnPropertyDescriptor(object, "visible");
    let keys = Object.keys(object);
    if (Object.hasOwn(object, "visible")) {
        total = total + descriptor.value + keys.length;
    }
}

total
