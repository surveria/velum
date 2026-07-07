let total = 0;
let proto = {
    shared: 1,
    bump: function(delta) {
        this.value += delta;
        return this.value + this.shared;
    },
};
let child = { __proto__: proto, value: 1 };

for (let index = 0; index < 4096; index++) {
    total += child.bump(1);
    total += child.shared;
    if ("bump" in child) {
        total += 1;
    }
    for (let key in child) {
        total += key === "value" ? 1 : 2;
    }
}

total
