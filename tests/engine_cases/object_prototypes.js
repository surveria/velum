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

print(inherited, method, own, restored);
print("shared" in child, "read" in child, "missing" in child);
print(keys);

child.__proto__ = null;
let cleared = child.shared;
print(cleared);

inherited === 40 &&
    method === 42 &&
    own === 41 &&
    restored === 40 &&
    ("shared" in child) === false &&
    cleared === undefined &&
    keys === "own;duplicate;shared;read;" ? 42 : 0
