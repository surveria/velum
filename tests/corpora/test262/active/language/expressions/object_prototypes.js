let proto = {
    shared: 40,
    duplicate: "proto",
    read: function(delta) {
        return this.own + this.shared + delta;
    },
};
let child = { __proto__: proto, own: 1, duplicate: "own" };

let inherited = child.shared;
let method = child.read(1);
child.shared = 41;
let own = child.shared;
delete child.shared;
let restored = child.shared;

let keys = "";
for (let key in child) {
    keys = keys + key + ";";
}

if (inherited !== 40 || method !== 42 || own !== 41 || restored !== 40) {
    throw new Test262Error("prototype lookup or shadowing was unexpected");
}
if (!("shared" in child) || !("read" in child) || "missing" in child) {
    throw new Test262Error("prototype membership was unexpected");
}
if (keys !== "own;duplicate;shared;read;") {
    throw new Test262Error("prototype enumeration was unexpected");
}

child.__proto__ = null;
if (child.shared !== undefined || "shared" in child) {
    throw new Test262Error("prototype clearing was unexpected");
}

42
